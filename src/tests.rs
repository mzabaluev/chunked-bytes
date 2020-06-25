use crate::{loosely, strictly, DrainChunks};
use bytes::{Buf, BufMut};

trait TestBuf: Buf + BufMut {
    fn with_chunk_size(size: usize) -> Self;
    fn drain_chunks(&mut self) -> DrainChunks<'_>;
    fn staging_capacity(&self) -> usize;
}

impl TestBuf for loosely::ChunkedBytes {
    fn with_chunk_size(size: usize) -> Self {
        loosely::ChunkedBytes::with_chunk_size_hint(size)
    }

    fn drain_chunks(&mut self) -> DrainChunks<'_> {
        self.drain_chunks()
    }

    fn staging_capacity(&self) -> usize {
        self.staging_capacity()
    }
}

impl TestBuf for strictly::ChunkedBytes {
    fn with_chunk_size(size: usize) -> Self {
        strictly::ChunkedBytes::with_chunk_size_limit(size)
    }

    fn drain_chunks(&mut self) -> DrainChunks<'_> {
        self.drain_chunks()
    }

    fn staging_capacity(&self) -> usize {
        self.staging_capacity()
    }
}

#[generic_tests::define]
mod properties {
    use super::*;

    #[test]
    fn reserve_does_not_grow_staging_buffer<B: TestBuf>() {
        let mut buf = B::with_chunk_size(8);
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
        assert_eq!(buf.staging_capacity(), cap);
        assert!(
            buf.drain_chunks().next().is_none(),
            "expected no chunks to be flushed"
        );
    }

    #[instantiate_tests(<loosely::ChunkedBytes>)]
    mod loosely_chunked_bytes {}

    #[instantiate_tests(<strictly::ChunkedBytes>)]
    mod strictly_chunked_bytes {}
}
