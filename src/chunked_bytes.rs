use bytes::{BufMut, Bytes, BytesMut};

use std::{
    cmp::{max, min},
    collections::VecDeque,
};

pub struct ChunkedBytes {
    current: BytesMut,
    chunks: VecDeque<Bytes>,
    chunk_size: usize,
}

impl ChunkedBytes {
    pub fn flush(&mut self) {
        let bytes = self.current.take();
        if !bytes.is_empty() {
            self.chunks.push_back(bytes.freeze())
        }
    }

    pub fn reserve(&mut self, additional: usize) {
        // If the current buffer has been taken from, its capacity
        // can be smaller than the chunk size. If a large capacity request
        // has been reserved, it can be larger. So we use the least of the two
        // as the limit for appending to the current buffer.
        let cap = min(self.current.capacity(), self.chunk_size);
        let written_len = self.current.len();
        let required = written_len.checked_add(additional).expect("overflow");
        if required > cap {
            self.flush();
            self.current.reserve(max(additional, self.chunk_size));
        }
    }

    pub fn append_chunk(&mut self, chunk: Bytes) {
        self.flush();
        if !chunk.is_empty() {
            self.chunks.push_back(chunk);
        }
    }
}

impl BufMut for ChunkedBytes {
    fn remaining_mut(&self) -> usize {
        self.current.remaining_mut()
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        self.current.advance_mut(cnt);
        if self.current.len() >= self.chunk_size {
            self.flush();
        }
    }

    unsafe fn bytes_mut(&mut self) -> &mut [u8] {
        self.current.bytes_mut()
    }
}
