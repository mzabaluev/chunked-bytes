use bytes::{Buf, BufMut, Bytes, BytesMut};
use iovec::IoVec;

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

impl Buf for ChunkedBytes {
    fn remaining(&self) -> usize {
        self.chunks.iter().fold(0, |acc, c| acc + c.len()) + self.current.len()
    }

    fn bytes(&self) -> &[u8] {
        if self.chunks.is_empty() {
            &self.current[..]
        } else {
            &self.chunks[0]
        }
    }

    fn advance(&mut self, mut cnt: usize) {
        loop {
            let chunk_len = match self.chunks.front_mut() {
                None => {
                    self.current.advance(cnt);
                    return;
                }
                Some(bytes) => {
                    let len = bytes.len();
                    if cnt < len {
                        bytes.advance(cnt);
                        return;
                    }
                    len
                }
            };
            cnt -= chunk_len;
            self.chunks.pop_front();
        }
    }

    fn bytes_vec<'a>(&'a self, dst: &mut [&'a IoVec]) -> usize {
        let n = {
            let zipped = dst.iter_mut().zip(self.chunks.iter());
            let len = zipped.len();
            for (iovec, chunk) in zipped {
                *iovec = (&chunk[..]).into();
            }
            len
        };

        if n < dst.len() && !self.current.is_empty() {
            dst[n] = (&self.current[..]).into();
            n + 1
        } else {
            n
        }
    }
}
