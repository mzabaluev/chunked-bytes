//! Buffer with a loose adherence to the preferred chunk size.

use super::chunked::Inner;
use crate::{DrainChunks, IntoChunks};

use bytes::buf::{Buf, BufMut, UninitSlice};
use bytes::Bytes;

use std::fmt;
use std::io::IoSlice;

/// A non-contiguous buffer for efficient serialization of data structures.
///
/// A `ChunkedBytes` container has a staging buffer to coalesce small byte
/// sequences of source data, and a queue of byte chunks split off the staging
/// buffer that can be incrementally consumed by an output API such as an object
/// implementing `AsyncWrite`. Once the number of bytes in the staging
/// buffer reaches a certain configured chunk size, the buffer content is
/// split off to form a new chunk.
///
/// This variant of the container does not enforce an upper limit on the size
/// of contiguous chunks, being optimized for performance. If your application
/// needs the sizes of the produced chunks to be capped,
/// use `strictly::ChunkedBytes` instead.
///
/// Refer to the documentation on the methods available for `ChunkedBytes`,
/// including the methods of traits `Buf` and `BufMut`, for details on working
/// with this container.
#[derive(Debug, Default)]
pub struct ChunkedBytes {
    inner: Inner,
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
    pub fn with_chunk_size_hint(chunk_size: usize) -> Self {
        ChunkedBytes {
            inner: Inner::with_chunk_size(chunk_size),
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
            inner: Inner::with_profile(chunk_size, chunking_capacity),
        }
    }

    /// Returns the size this `ChunkedBytes` container uses as the threshold
    /// for splitting off complete chunks.
    ///
    /// Note that the size of produced chunks may be larger or smaller than the
    /// configured value, due to the allocation strategy used internally by
    /// the implementation and also depending on the pattern of usage.
    #[inline]
    pub fn chunk_size_hint(&self) -> usize {
        self.inner.chunk_size()
    }

    /// Returns true if the `ChunkedBytes` container has no complete chunks
    /// and the staging buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[cfg(test)]
    pub fn staging_capacity(&self) -> usize {
        self.inner.staging_capacity()
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
        self.inner.flush()
    }

    /// Appends a `Bytes` slice to the container without copying the data.
    ///
    /// If `chunk` is empty, this method does nothing. Otherwise,
    /// if there are any bytes currently in the staging buffer, they are split
    /// to form a complete chunk. Next, the given slice is appended as the
    /// next chunk.
    ///
    /// # Performance Notes
    ///
    /// For a small slice originating from a buffer that is not split
    /// or shared between other `Bytes` instances, copying the bytes with
    /// `BufMut::put_slice` may be faster than the overhead of
    /// atomic reference counting induced by use of this method.
    #[inline]
    pub fn put_bytes(&mut self, chunk: Bytes) {
        if !chunk.is_empty() {
            self.flush();
            self.inner.push_chunk(chunk);
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
        self.inner.drain_chunks()
    }

    /// Consumes the `ChunkedBytes` container to produce an iterator over
    /// its chunks. If there are bytes in the staging buffer, they are yielded
    /// as the last chunk.
    ///
    /// The memory allocated for `IntoChunks` may be slightly more than the
    /// `ChunkedBytes` container it consumes. This is an infrequent side effect
    /// of making the internal state efficient in general for iteration.
    #[inline]
    pub fn into_chunks(self) -> IntoChunks {
        self.inner.into_chunks()
    }
}

unsafe impl BufMut for ChunkedBytes {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.inner.remaining_mut()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.inner.advance_mut(cnt);
    }

    /// Returns a mutable slice of unwritten bytes available in
    /// the staging buffer, starting at the current writing position.
    ///
    /// The length of the slice may be larger than the preferred chunk
    /// size due to the allocation strategy used internally by
    /// the implementation.
    #[inline]
    fn bytes_mut(&mut self) -> &mut UninitSlice {
        if self.inner.staging_len() == self.inner.staging_capacity() {
            self.inner.reserve_staging();
        }
        self.inner.bytes_mut()
    }
}

impl Buf for ChunkedBytes {
    #[inline]
    fn remaining(&self) -> usize {
        self.inner.remaining()
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
        self.inner.bytes()
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
    #[inline]
    fn advance(&mut self, cnt: usize) {
        let _ = self.inner.advance(cnt);
    }

    /// Fills `dst` sequentially with the slice views of the chunks, then
    /// the bytes in the staging buffer if any remain and there is
    /// another unfilled entry left in `dst`. Returns the number of `IoSlice`
    /// entries filled.
    #[inline]
    fn bytes_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        self.inner.bytes_vectored(dst)
    }

    #[inline]
    fn copy_to_bytes(&mut self, len: usize) -> Bytes {
        self.inner.copy_to_bytes(len)
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
