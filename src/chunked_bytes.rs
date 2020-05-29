use crate::{DrainChunks, IntoChunks};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::cmp::max;
use std::collections::VecDeque;
use std::io::IoSlice;
use std::mem::MaybeUninit;

const DEFAULT_CHUNK_SIZE: usize = 4096;

#[derive(Debug)]
pub struct ChunkedBytes {
    staging: BytesMut,
    chunks: VecDeque<Bytes>,
    chunk_size: usize,
}

impl Default for ChunkedBytes {
    fn default() -> Self {
        ChunkedBytes {
            staging: BytesMut::new(),
            chunks: VecDeque::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }
}

impl ChunkedBytes {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_chunk_size(chunk_size: usize) -> Self {
        ChunkedBytes {
            chunk_size,
            ..Default::default()
        }
    }

    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.staging.is_empty()
    }

    /// Splits any bytes that have accumulated in the staging buffer
    /// into a new complete chunk. If the staging buffer is empty, this method
    /// does nothing.
    pub fn flush(&mut self) {
        if !self.staging.is_empty() {
            let bytes = self.staging.split().freeze();
            self.chunks.push_back(bytes)
        }
    }

    /// Reserves capacity for at least `additional` bytes
    /// to be appended contiguously to the staging buffer.
    /// The next call to `bytes_mut` will return a slice of at least
    /// this size. Note that this may be larger than the chunk size.
    ///
    /// Bytes written previously to the staging buffer are split off to
    /// a new complete chunk if the added capacity would have caused
    /// the staging buffer to grow beyond the nominal chunk size.
    pub fn reserve(&mut self, mut additional: usize) {
        let written_len = self.staging.len();
        let required = written_len.checked_add(additional).expect("overflow");
        if required > self.chunk_size {
            self.flush();
            additional = max(additional, self.chunk_size);
        }
        self.staging.reserve(additional);
    }

    pub fn append_chunk(&mut self, chunk: Bytes) {
        if !chunk.is_empty() {
            self.flush();
            self.chunks.push_back(chunk);
        }
    }

    pub fn drain_chunks(&mut self) -> DrainChunks<'_> {
        DrainChunks::new(self.chunks.drain(..))
    }

    pub fn into_chunks(mut self) -> IntoChunks {
        if !self.staging.is_empty() {
            self.chunks.push_back(self.staging.freeze());
        }
        IntoChunks::new(self.chunks.into_iter())
    }
}

impl BufMut for ChunkedBytes {
    fn remaining_mut(&self) -> usize {
        self.staging.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.staging.advance_mut(cnt);
        if self.staging.len() >= self.chunk_size {
            self.flush();
        }
    }

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

    fn has_remaining(&self) -> bool {
        !self.is_empty()
    }

    fn bytes(&self) -> &[u8] {
        if let Some(chunk) = self.chunks.front() {
            chunk
        } else {
            &self.staging
        }
    }

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
