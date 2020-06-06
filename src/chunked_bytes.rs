use crate::{DrainChunks, IntoChunks};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::collections::VecDeque;
use std::fmt;
use std::io::IoSlice;
use std::mem::MaybeUninit;

const DEFAULT_CHUNK_SIZE: usize = 4096;

/// A non-contiguous buffer for efficient serialization of data structures.
///
/// A `ChunkedBytes` container has a staging buffer to coalesce small byte
/// sequences of source data, and a queue of byte chunks split off the staging
/// buffer that can be incrementally consumed by an output API such as an object
/// implementing `AsyncWrite`. Once the number of bytes in the staging
/// buffer reaches a certain configured chunk size, the buffer content is
/// split off to form a new chunk.
///
/// Refer to the documentation on the methods available for `ChunkedBytes`,
/// including the methods of traits `Buf` and `BufMut`, for details on working
/// with this container.
#[derive(Debug)]
pub struct ChunkedBytes {
    staging: BytesMut,
    chunks: VecDeque<Bytes>,
    chunk_size: usize,
}

impl Default for ChunkedBytes {
    #[inline]
    fn default() -> Self {
        ChunkedBytes {
            staging: BytesMut::new(),
            chunks: VecDeque::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }
}

impl ChunkedBytes {
    /// Creates a new `ChunkedBytes` container with the preferred chunk size
    /// set to a default value.
    #[inline]
    pub fn new() -> Self {
        Default::default()
    }

    /// Creates a new `ChunkedBytes` container with the given chunk size
    /// to prefer.
    #[inline]
    pub fn with_preferred_chunk_size(chunk_size: usize) -> Self {
        ChunkedBytes {
            chunk_size,
            ..Default::default()
        }
    }

    /// The fully detailed constructor for `ChunkedBytes`.
    /// The preferred chunk size is given in `chunk_size`, and an upper
    /// estimate of the number of chunks this container could be expected to
    /// have at any moment of time should be given in `chunking_capacity`.
    /// More chunks can still be held, but this may cause reallocations of
    /// internal data structures.
    #[inline]
    pub fn with_profile(chunk_size: usize, chunking_capacity: usize) -> Self {
        ChunkedBytes {
            staging: BytesMut::new(),
            chunks: VecDeque::with_capacity(chunking_capacity),
            chunk_size,
        }
    }

    /// Returns the size this `ChunkedBytes` container uses as the threshold
    /// for splitting off complete chunks.
    ///
    /// Note that the size of produced chunks may be larger or smaller than the
    /// configured value, due to the allocation strategy used internally by
    /// the implementation and also depending on the pattern of usage.
    #[inline]
    pub fn preferred_chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Returns true if the `ChunkedBytes` container has no complete chunks
    /// and the staging buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.staging.is_empty()
    }

    /// Splits any bytes that are currently in the staging buffer into a new
    /// complete chunk.
    /// If the staging buffer is empty, this method does nothing.
    ///
    /// Most users should not need to call this method. It is called
    /// internally when needed by the methods that advance the writing
    /// position.
    #[inline]
    pub fn flush(&mut self) {
        if !self.staging.is_empty() {
            let bytes = self.staging.split().freeze();
            self.chunks.push_back(bytes)
        }
    }

    /// Appends a `Bytes` slice to the container without copying the data.
    ///
    /// If there are any bytes currently in the staging buffer, they are split
    /// to form a complete chunk. Next, the given slice is appended as the
    /// next chunk.
    #[inline]
    pub fn push_chunk(&mut self, chunk: Bytes) {
        if !chunk.is_empty() {
            self.flush();
            self.chunks.push_back(chunk);
        }
    }

    /// Returns an iterator that removes complete chunks from the
    /// `ChunkedBytes` container and yields the removed chunks as `Bytes`
    /// slice handles. This does not include bytes in the staging buffer.
    ///
    /// The chunks are removed even if the iterator is dropped without being
    /// consumed until the end. It is unspecified how many chunks are removed
    /// if the `DrainChunks` value is not dropped, but the borrow it holds
    /// expires (e.g. due to `std::mem::forget`).
    #[inline]
    pub fn drain_chunks(&mut self) -> DrainChunks<'_> {
        DrainChunks::new(self.chunks.drain(..))
    }

    /// Consumes the `ChunkedBytes` container to produce an iterator over
    /// its chunks. If there are bytes in the staging buffer, they are yielded
    /// as the last chunk.
    ///
    /// The memory allocated for `IntoChunks` may be slightly more than the
    /// `ChunkedBytes` container it consumes. This is an infrequent side effect
    /// of making the internal state efficient in general for iteration.
    #[inline]
    pub fn into_chunks(mut self) -> IntoChunks {
        if !self.staging.is_empty() {
            self.chunks.push_back(self.staging.freeze());
        }
        IntoChunks::new(self.chunks.into_iter())
    }

    fn reserve_staging(&mut self) {
        let cap = self.staging.capacity();
        debug_assert_eq!(cap, self.staging.len());

        // We are here when either:
        // a) the buffer has never been used and never allocated;
        // b) the producer has filled a previously allocated buffer,
        //    and the consumer may have read a part or the whole of it.
        // Our goal is to reserve space in the staging buffer without
        // forcing it to reallocate to a larger capacity.
        //
        // To reuse the allocation of `BytesMut` in the vector form with
        // the offset `off` and remaining capacity `cap` while reserving
        // `additional` bytes, the following needs to apply:
        //
        // off >= additional && off >= cap / 2
        //
        // We have:
        //
        // off + cap == allocated_size >= chunk_size
        //
        // From this, we can derive the following condition check:
        let cutoff = cap.saturating_add(cap / 2);
        let additional = if cutoff > self.chunk_size {
            // Alas, the bytes still in the staging buffer are likely to
            // necessitate a new allocation. Split them off to a chunk
            // first, so that the new allocation does not have to copy
            // them and the total required capacity is `self.chunk_size`.
            self.flush();
            self.chunk_size
        } else {
            // This amount will get BytesMut to reuse the allocation and
            // copy back the bytes if there are no chunks left unconsumed.
            // Otherwise, it will reallocate to its previous capacity.
            // A virgin buffer will be allocated to `self.chunk_size`.
            self.chunk_size - cap
        };
        self.staging.reserve(additional);
    }
}

impl BufMut for ChunkedBytes {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.staging.remaining_mut()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.staging.advance_mut(cnt);
    }

    /// Returns a mutable slice of unwritten bytes available in
    /// the staging buffer, starting at the current writing position.
    ///
    /// The length of the slice may be larger than the preferred chunk
    /// size due to the allocation strategy used internally by
    /// the implementation.
    #[inline]
    fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        if self.staging.len() == self.staging.capacity() {
            self.reserve_staging();
        }
        self.staging.bytes_mut()
    }
}

impl Buf for ChunkedBytes {
    fn remaining(&self) -> usize {
        self.chunks
            .iter()
            .fold(self.staging.len(), |sum, chunk| sum + chunk.len())
    }

    #[inline]
    fn has_remaining(&self) -> bool {
        !self.is_empty()
    }

    /// Returns a slice of the bytes in the first extant complete chunk,
    /// or the bytes in the staging buffer if there are no unconsumed chunks.
    ///
    /// It is more efficient to use `bytes_vectored` to gather all the disjoint
    /// slices for vectored output, as is done in many specialized
    /// implementations of the `AsyncWrite::poll_write_buf` method.
    #[inline]
    fn bytes(&self) -> &[u8] {
        if let Some(chunk) = self.chunks.front() {
            chunk
        } else {
            &self.staging
        }
    }

    /// Advances the reading position by `cnt`, dropping the `Bytes` references
    /// to any complete chunks that the position has been advanced past
    /// and then advancing the starting position of the first remaining chunk.
    /// If there are no complete chunks left, the reading position is advanced
    /// in the staging buffer, effectively removing the consumed bytes.
    ///
    /// # Panics
    ///
    /// This function may panic when `cnt > self.remaining()`.
    ///
    fn advance(&mut self, mut cnt: usize) {
        loop {
            match self.chunks.front_mut() {
                None => {
                    self.staging.advance(cnt);
                    return;
                }
                Some(chunk) => {
                    let len = chunk.len();
                    if cnt < len {
                        chunk.advance(cnt);
                        return;
                    } else {
                        cnt -= len;
                        self.chunks.pop_front();
                    }
                }
            }
        }
    }

    /// Fills `dst` sequentially with the slice views of the chunks, then
    /// the bytes in the staging buffer if any remain and there is
    /// another unfilled entry left in `dst`. Returns the number of `IoSlice`
    /// entries filled.
    fn bytes_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        let n = {
            let zipped = dst.iter_mut().zip(self.chunks.iter());
            let len = zipped.len();
            for (io_slice, chunk) in zipped {
                *io_slice = IoSlice::new(chunk);
            }
            len
        };

        if n < dst.len() && !self.staging.is_empty() {
            dst[n] = IoSlice::new(&self.staging);
            n + 1
        } else {
            n
        }
    }

    fn to_bytes(&mut self) -> Bytes {
        match self.chunks.pop_front() {
            None => self.staging.split().freeze(),
            Some(chunk) => {
                if self.is_empty() {
                    return chunk;
                }
                let cap = chunk.len() + self.remaining();
                let mut buf = BytesMut::with_capacity(cap);
                buf.put(chunk);
                while let Some(chunk) = self.chunks.pop_front() {
                    buf.put(chunk);
                }
                buf.put(self.staging.split());
                buf.freeze()
            }
        }
    }
}

impl fmt::Write for ChunkedBytes {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.remaining_mut() >= s.len() {
            self.put_slice(s.as_bytes());
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }

    // The default implementation delegates to
    // fmt::write(&mut self as &mut dyn fmt::Write, args)
    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        fmt::write(self, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_does_not_grow_staging_buffer() {
        let mut buf = ChunkedBytes::with_preferred_chunk_size(8);
        let cap = buf.bytes_mut().len();
        assert!(cap >= 8);

        buf.put(&vec![0; cap][..]);
        assert_eq!(buf.bytes_mut().len(), cap);
        {
            let mut chunks = buf.drain_chunks();
            let chunk = chunks.next().expect("expected a chunk to be flushed");
            assert_eq!(chunk.len(), cap);
            assert!(chunks.next().is_none());
        }

        buf.put(&vec![0; cap - 4][..]);
        buf.advance(cap - 6);
        buf.put(&[0; 4][..]);
        assert_eq!(buf.bytes_mut().len(), cap);
        {
            let mut chunks = buf.drain_chunks();
            let chunk = chunks.next().expect("expected a chunk to be flushed");
            assert_eq!(chunk.len(), 6);
            assert!(chunks.next().is_none());
        }

        buf.put(&vec![0; cap - 5][..]);
        buf.advance(cap - 5);
        buf.put(&[0; 5][..]);
        assert_eq!(buf.bytes_mut().len(), cap - 5);
        assert_eq!(buf.staging.capacity(), cap);
        assert!(
            buf.drain_chunks().next().is_none(),
            "expected no chunks to be flushed"
        );
    }
}
