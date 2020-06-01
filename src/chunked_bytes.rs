use crate::{DrainChunks, IntoChunks};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::collections::VecDeque;
use std::io::IoSlice;
use std::mem::MaybeUninit;

const DEFAULT_CHUNK_SIZE: usize = 4096;
const INITIAL_CHUNKS_CAPACITY: usize = 1;

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
            chunks: VecDeque::with_capacity(INITIAL_CHUNKS_CAPACITY),
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

    /// Returns the size this `ChunkedBytes` container uses as the threshold
    /// for splitting off complete chunks.
    ///
    /// Note that the size of produced chunks may be larger than the
    /// configured value due to the allocation strategy used internally by
    /// the implementation. Chunks may also be smaller than the threshold if
    /// writing with `BufMut` methods has been mixed with use of the
    /// `push_chunk` method, or the `flush` method has been called directly.
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
}

impl BufMut for ChunkedBytes {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.staging.remaining_mut()
    }

    /// Advances the writing position in the staging buffer.
    ///
    /// If the number of bytes accumulated in the staging buffer reaches
    /// or exceeds the preferred chunk size, the bytes are split off
    /// to form a new complete chunk.
    ///
    /// # Panics
    ///
    /// This function may panic if `cnt > self.remaining_mut()`.
    ///
    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.staging.advance_mut(cnt);
        if self.staging.len() >= self.chunk_size {
            self.flush();
        }
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
            self.flush();
            self.staging.reserve(self.chunk_size);
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
