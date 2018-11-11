use bytes::{BufMut, Bytes, BytesMut};

use std::collections::VecDeque;

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
        if self.current.remaining_mut() + additional > self.chunk_size {
            self.flush();
        }
        self.current.reserve(additional);
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
