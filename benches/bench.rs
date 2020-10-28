#![feature(test)]
#![feature(maybe_uninit_slice)]
#![feature(write_all_vectored)]

extern crate test;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use chunked_bytes::{loosely, strictly};

use std::cmp::min;
use std::io::{self, IoSlice, Write};
use std::ptr;
use test::Bencher;

/// Imitates default TCP socket buffer size on Linux
const BUF_SIZE: usize = 16 * 1024;

trait BenchBuf: Buf + BufMut {
    fn construct() -> Self;
    fn construct_with_profile(chunk_size: usize, cnt: usize) -> Self;
    fn put_bytes(&mut self, bytes: Bytes);

    fn produce(&mut self, mut cnt: usize) {
        while cnt != 0 {
            let dst = self.bytes_mut();
            let write_len = min(cnt, dst.len());
            unsafe {
                ptr::write_bytes(dst.as_mut_ptr(), 0, write_len);
                self.advance_mut(write_len);
            }
            cnt -= write_len;
        }
    }

    fn consume_vectored(&mut self, mut cnt: usize) {
        // Do what TcpStream does
        loop {
            let mut slices = [IoSlice::new(&[]); 64];
            let n = self.bytes_vectored(&mut slices);
            if n == 0 {
                break;
            }
            let mut sink = io::sink();
            let total_len = sink.write_vectored(&mut slices[..n]).unwrap();
            if cnt <= total_len {
                self.advance(cnt);
                break;
            } else {
                self.advance(total_len);
                cnt -= total_len;
            }
        }
    }
}

#[allow(dead_code)]
fn write_vectored_into_black_box<B: Buf>(buf: &mut B, mut cnt: usize) {
    // Do what TcpStream does
    loop {
        let mut slices = [IoSlice::new(&[]); 64];
        let n = buf.bytes_vectored(&mut slices);
        if n == 0 {
            break;
        }
        let mut total_len = 0;
        for i in 0..n {
            let slice = &*slices[i];
            total_len += slice.len();
            // This has heavy impact on the benchmarks. Vectored output calls
            // to the OS would not make that big of a difference.
            for p in slice {
                test::black_box(*p);
            }
        }
        if cnt <= total_len {
            buf.advance(cnt);
            break;
        } else {
            buf.advance(total_len);
            cnt -= total_len;
        }
    }
}

impl BenchBuf for loosely::ChunkedBytes {
    fn construct() -> Self {
        Self::with_chunk_size_hint(BUF_SIZE)
    }

    fn construct_with_profile(chunk_size: usize, cnt: usize) -> Self {
        Self::with_profile(chunk_size, cnt)
    }

    fn put_bytes(&mut self, bytes: Bytes) {
        self.put_bytes(bytes)
    }
}

impl BenchBuf for strictly::ChunkedBytes {
    fn construct() -> Self {
        Self::with_chunk_size_limit(BUF_SIZE)
    }

    fn construct_with_profile(chunk_size: usize, cnt: usize) -> Self {
        Self::with_profile(chunk_size, cnt)
    }

    fn put_bytes(&mut self, bytes: Bytes) {
        self.put_bytes(bytes)
    }
}

impl BenchBuf for BytesMut {
    fn construct() -> Self {
        BytesMut::with_capacity(BUF_SIZE)
    }

    fn construct_with_profile(chunk_size: usize, cnt: usize) -> Self {
        BytesMut::with_capacity(chunk_size * cnt)
    }

    fn put_bytes(&mut self, bytes: Bytes) {
        self.put(bytes)
    }
}

#[generic_tests::define]
mod benches {
    use super::*;

    #[bench]
    fn clean_pass_through<B: BenchBuf>(b: &mut Bencher) {
        let mut buf = B::construct();
        let prealloc_cap = buf.bytes_mut().len();
        b.iter(|| {
            buf.produce(prealloc_cap);
            buf.consume_vectored(prealloc_cap);
        });
    }

    fn pump_through_staggered<B: BenchBuf>(b: &mut Bencher, carry_over: usize) {
        let mut buf = B::construct();
        let prealloc_cap = buf.bytes_mut().len();
        buf.produce(prealloc_cap);
        b.iter(|| {
            buf.consume_vectored(prealloc_cap - carry_over);
            buf.produce(prealloc_cap - carry_over);
        });
    }

    #[bench]
    fn staggered_copy_back<B: BenchBuf>(b: &mut Bencher) {
        pump_through_staggered::<B>(b, BUF_SIZE * 2 / 3);
    }

    #[bench]
    fn staggered_new_alloc<B: BenchBuf>(b: &mut Bencher) {
        pump_through_staggered::<B>(b, (BUF_SIZE * 2 + 2) / 3 + 1);
    }

    fn pump_pressured<B: BenchBuf>(
        inflow: usize,
        outflow: usize,
        b: &mut Bencher,
    ) {
        let mut buf = B::construct();
        let prealloc_cap = buf.bytes_mut().len();
        b.iter(|| {
            buf.produce(inflow);
            while buf.remaining() >= prealloc_cap {
                buf.consume_vectored(outflow);
            }
        });
    }

    #[bench]
    fn pressured_in_50_out_50_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE / 2, BUF_SIZE / 2, b);
    }

    #[bench]
    fn pressured_in_300_out_50_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 3, BUF_SIZE / 2, b);
    }

    #[bench]
    fn pressured_in_310_out_50_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 3 + BUF_SIZE / 10, BUF_SIZE / 2, b);
    }

    #[bench]
    fn pressured_in_350_out_50_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 3 + BUF_SIZE / 2, BUF_SIZE / 2, b);
    }

    #[bench]
    fn pressured_in_900_out_50_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 9, BUF_SIZE / 2, b);
    }

    #[bench]
    fn pressured_in_150_out_100_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE + BUF_SIZE / 2, BUF_SIZE, b);
    }

    #[bench]
    fn pressured_in_200_out_100_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 2, BUF_SIZE, b);
    }

    #[bench]
    fn pressured_in_210_out_100_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 2 + BUF_SIZE / 10, BUF_SIZE, b);
    }

    #[bench]
    fn pressured_in_300_out_100_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 3, BUF_SIZE, b);
    }

    #[bench]
    fn pressured_in_900_out_100_percent<B: BenchBuf>(b: &mut Bencher) {
        pump_pressured::<B>(BUF_SIZE * 9, BUF_SIZE, b);
    }

    fn pass_bytes_through<B: BenchBuf>(
        b: &mut Bencher,
        chunk_size: usize,
        cnt: usize,
    ) {
        let mut buf = B::construct_with_profile(chunk_size, cnt);
        b.iter(|| {
            let mut salami = Bytes::from(vec![0; chunk_size * cnt]);
            for _ in 0..cnt {
                buf.put_bytes(salami.split_to(chunk_size));
            }
            while buf.has_remaining() {
                buf.consume_vectored(BUF_SIZE);
            }
        });
    }

    #[bench]
    fn pass_bytes_through_sliced_by_16<B: BenchBuf>(b: &mut Bencher) {
        pass_bytes_through::<B>(b, BUF_SIZE / 16, 16);
    }

    #[bench]
    fn pass_bytes_through_sliced_by_64<B: BenchBuf>(b: &mut Bencher) {
        pass_bytes_through::<B>(b, BUF_SIZE / 64, 64);
    }

    #[bench]
    fn pass_bytes_through_sliced_by_256<B: BenchBuf>(b: &mut Bencher) {
        pass_bytes_through::<B>(b, BUF_SIZE / 256, 256);
    }

    #[bench]
    fn pass_bytes_through_medium_sized<B: BenchBuf>(b: &mut Bencher) {
        pass_bytes_through::<B>(b, BUF_SIZE / 4, 16);
    }

    #[bench]
    fn pass_bytes_through_larger_than_buf<B: BenchBuf>(b: &mut Bencher) {
        pass_bytes_through::<B>(b, BUF_SIZE * 2, 2);
    }

    fn mix_slice_and_bytes<B: BenchBuf>(
        b: &mut Bencher,
        slice_len: usize,
        bytes_len: usize,
    ) {
        let mut buf = B::construct();
        let v = vec![0; slice_len];
        b.iter(|| {
            buf.put_slice(&v);
            buf.put_bytes(Bytes::from(vec![0; bytes_len]));
            while buf.has_remaining() {
                buf.consume_vectored(BUF_SIZE);
            }
        });
    }

    #[bench]
    fn mix_slice_and_bytes_32_32<B: BenchBuf>(b: &mut Bencher) {
        mix_slice_and_bytes::<B>(b, 32, 32)
    }

    #[bench]
    fn mix_slice_and_bytes_32_4096<B: BenchBuf>(b: &mut Bencher) {
        mix_slice_and_bytes::<B>(b, 32, 4096)
    }

    #[instantiate_tests(<loosely::ChunkedBytes>)]
    mod loosely_chunked_bytes {}

    #[instantiate_tests(<strictly::ChunkedBytes>)]
    mod strictly_chunked_bytes {}

    #[instantiate_tests(<BytesMut>)]
    mod bytes_mut {}
}
