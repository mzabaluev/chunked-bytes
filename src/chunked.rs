use crate::{DrainChunks, IntoChunks};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use std::collections::VecDeque;
use std::io::IoSlice;
use std::mem::MaybeUninit;

const DEFAULT_CHUNK_SIZE: usize = 4096;

#[derive(Debug)]
pub(crate) struct Inner {
    staging: BytesMut,
    chunks: VecDeque<Bytes>,
    chunk_size: usize,
}

impl Default for Inner {
    #[inline]
    fn default() -> Self {
        Inner {
            staging: BytesMut::new(),
            chunks: VecDeque::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
        }
    }
}

pub(crate) enum AdvanceStopped {
    InChunk,
    InStaging(usize),
}

impl Inner {
    #[inline]
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Inner {
            chunk_size,
            ..Default::default()
        }
    }

    #[inline]
    pub fn with_profile(chunk_size: usize, chunking_capacity: usize) -> Self {
        Inner {
            staging: BytesMut::new(),
            chunks: VecDeque::with_capacity(chunking_capacity),
            chunk_size,
        }
    }

    #[inline]
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty() && self.staging.is_empty()
    }

    #[inline]
    pub fn staging_len(&self) -> usize {
        self.staging.len()
    }

    #[inline]
    pub fn staging_capacity(&self) -> usize {
        self.staging.capacity()
    }

    #[inline]
    pub fn push_chunk(&mut self, chunk: Bytes) {
        debug_assert!(!chunk.is_empty());
        self.chunks.push_back(chunk)
    }

    #[inline]
    pub fn flush(&mut self) {
        if !self.staging.is_empty() {
            let bytes = self.staging.split().freeze();
            self.push_chunk(bytes)
        }
    }

    #[inline]
    pub fn drain_chunks(&mut self) -> DrainChunks<'_> {
        DrainChunks::new(self.chunks.drain(..))
    }

    #[inline]
    pub fn into_chunks(mut self) -> IntoChunks {
        if !self.staging.is_empty() {
            self.chunks.push_back(self.staging.freeze());
        }
        IntoChunks::new(self.chunks.into_iter())
    }

    pub fn reserve_staging(&mut self) -> usize {
        let cap = self.staging.capacity();

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
        self.staging.capacity()
    }

    #[inline]
    pub fn remaining_mut(&self) -> usize {
        self.staging.remaining_mut()
    }

    #[inline]
    pub unsafe fn advance_mut(&mut self, cnt: usize) {
        self.staging.advance_mut(cnt);
    }

    #[inline]
    pub fn bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        self.staging.bytes_mut()
    }

    pub fn remaining(&self) -> usize {
        self.chunks
            .iter()
            .fold(self.staging.len(), |sum, chunk| sum + chunk.len())
    }

    #[inline]
    pub fn bytes(&self) -> &[u8] {
        if let Some(chunk) = self.chunks.front() {
            chunk
        } else {
            &self.staging
        }
    }

    pub fn advance(&mut self, mut cnt: usize) -> AdvanceStopped {
        loop {
            match self.chunks.front_mut() {
                None => {
                    self.staging.advance(cnt);
                    return AdvanceStopped::InStaging(cnt);
                }
                Some(chunk) => {
                    let len = chunk.len();
                    if cnt < len {
                        chunk.advance(cnt);
                        return AdvanceStopped::InChunk;
                    } else {
                        cnt -= len;
                        self.chunks.pop_front();
                    }
                }
            }
        }
    }

    pub fn bytes_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
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

    pub fn take_bytes(&mut self) -> Bytes {
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
