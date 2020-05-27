use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::cmp::{max, min};
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

    pub fn flush(&mut self) {
        if !self.staging.is_empty() {
            let bytes = self.staging.split().freeze();
            self.chunks.push_back(bytes)
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        // If the staging buffer has been taken from, its capacity
        // can be smaller than the chunk size. If a large capacity request
        // has been reserved, it can be larger. So we use the least of the two
        // as the limit for appending to the staging buffer.
        let cap = min(self.staging.capacity(), self.chunk_size);
        let written_len = self.staging.len();
        let required = written_len.checked_add(additional).expect("overflow");
        if required > cap {
            self.flush();
            self.staging.reserve(max(additional, self.chunk_size));
        }
    }

    pub fn append_chunk(&mut self, chunk: Bytes) {
        if !chunk.is_empty() {
            self.flush();
            self.chunks.push_back(chunk);
        }
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
        self.staging.bytes_mut()
    }
}

impl Buf for ChunkedBytes {
    fn remaining(&self) -> usize {
        self.chunks.iter().fold(0, |acc, c| acc + c.len()) + self.staging.len()
    }

    fn bytes(&self) -> &[u8] {
        if self.chunks.is_empty() {
            &self.staging[..]
        } else {
            &self.chunks[0]
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
