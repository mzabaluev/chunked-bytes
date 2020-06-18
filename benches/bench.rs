#![feature(test)]
#![feature(maybe_uninit_slice)]
#![feature(write_all_vectored)]

extern crate test;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use chunked_bytes::ChunkedBytes;

use std::cmp::min;
use std::io::{self, IoSlice, Write};
use std::mem::MaybeUninit;
use std::ptr;
use test::Bencher;

/// Imitates default TCP socket buffer size on Linux
const BUF_SIZE: usize = 16 * 1024;

fn produce<B: BufMut>(buf: &mut B, mut cnt: usize) {
    while cnt != 0 {
        let dst = buf.bytes_mut();
        let write_len = min(cnt, dst.len());
        unsafe {
            ptr::write_bytes(MaybeUninit::first_ptr_mut(dst), 0, write_len);
            buf.advance_mut(write_len);
        }
        cnt -= write_len;
    }
}

fn consume_vectored<B: Buf>(buf: &mut B, mut cnt: usize) {
    // Do what TcpStream does
    loop {
        let mut slices = [IoSlice::new(&[]); 64];
        let n = buf.bytes_vectored(&mut slices);
        if n == 0 {
            break;
        }
        let mut sink = io::sink();
        let total_len = sink.write_vectored(&mut slices[..n]).unwrap();
        if cnt <= total_len {
            buf.advance(cnt);
            break;
        } else {
            buf.advance(total_len);
            cnt -= total_len;
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

fn pump_through_clean<B: Buf + BufMut>(b: &mut Bencher, mut buf: B) {
    let prealloc_cap = buf.bytes_mut().len();
    b.iter(|| {
        produce(&mut buf, prealloc_cap);
        consume_vectored(&mut buf, prealloc_cap);
    });
}

#[bench]
fn clean_pass_through_chunked(b: &mut Bencher) {
    pump_through_clean(b, ChunkedBytes::with_chunk_size_hint(BUF_SIZE));
}

#[bench]
fn clean_pass_through_straight(b: &mut Bencher) {
    pump_through_clean(b, BytesMut::with_capacity(BUF_SIZE));
}

fn pump_through_staggered<B: Buf + BufMut>(
    b: &mut Bencher,
    mut buf: B,
    carry_over: usize,
) {
    let prealloc_cap = buf.bytes_mut().len();
    produce(&mut buf, prealloc_cap);
    b.iter(|| {
        consume_vectored(&mut buf, prealloc_cap - carry_over);
        produce(&mut buf, prealloc_cap - carry_over);
    });
}

#[bench]
fn staggered_copy_back_chunked(b: &mut Bencher) {
    pump_through_staggered(
        b,
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 2 / 3,
    );
}

#[bench]
fn staggered_copy_back_straight(b: &mut Bencher) {
    pump_through_staggered(
        b,
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 2 / 3,
    );
}

#[bench]
fn staggered_new_alloc_chunked(b: &mut Bencher) {
    pump_through_staggered(
        b,
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        (BUF_SIZE * 2 + 2) / 3 + 1,
    );
}

#[bench]
fn staggered_new_alloc_straight(b: &mut Bencher) {
    pump_through_staggered(
        b,
        BytesMut::with_capacity(BUF_SIZE),
        (BUF_SIZE * 2 + 2) / 3 + 1,
    );
}

fn pump_pressured<B: Buf + BufMut>(
    mut buf: B,
    inflow: usize,
    outflow: usize,
    b: &mut Bencher,
) {
    let prealloc_cap = buf.bytes_mut().len();
    b.iter(|| {
        produce(&mut buf, inflow);
        while buf.remaining() >= prealloc_cap {
            consume_vectored(&mut buf, outflow);
        }
    });
}

#[bench]
fn pressured_in_50_out_50_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE / 2,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_50_out_50_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE / 2,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_300_out_50_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 3,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_300_out_50_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 3,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_310_out_50_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 3 + BUF_SIZE / 10,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_310_out_50_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 3 + BUF_SIZE / 10,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_350_out_50_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 3 + BUF_SIZE / 2,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_350_out_50_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 3 + BUF_SIZE / 2,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_900_out_50_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 9,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_900_out_50_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 9,
        BUF_SIZE / 2,
        b,
    );
}

#[bench]
fn pressured_in_150_out_100_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE + BUF_SIZE / 2,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_150_out_100_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE + BUF_SIZE / 2,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_200_out_100_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 2,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_200_out_100_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 2,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_210_out_100_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 2 + BUF_SIZE / 10,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_210_out_100_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 2 + BUF_SIZE / 10,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_300_out_100_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_chunk_size_hint(BUF_SIZE),
        BUF_SIZE * 3,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_300_out_100_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 3,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_900_out_100_percent_chunked(b: &mut Bencher) {
    pump_pressured(
        ChunkedBytes::with_profile(BUF_SIZE, 9),
        BUF_SIZE * 9,
        BUF_SIZE,
        b,
    );
}

#[bench]
fn pressured_in_900_out_100_percent_straight(b: &mut Bencher) {
    pump_pressured(
        BytesMut::with_capacity(BUF_SIZE),
        BUF_SIZE * 9,
        BUF_SIZE,
        b,
    );
}

fn pass_bytes_through_chunked(b: &mut Bencher, chunk_size: usize, cnt: usize) {
    let mut buf = ChunkedBytes::with_profile(chunk_size, cnt);
    b.iter(|| {
        let mut salami = Bytes::from(vec![0; chunk_size * cnt]);
        for _ in 0..cnt {
            buf.put_bytes(salami.split_to(chunk_size));
        }
        while buf.has_remaining() {
            consume_vectored(&mut buf, BUF_SIZE);
        }
    });
}

fn pass_bytes_through_straight(b: &mut Bencher, chunk_size: usize, cnt: usize) {
    let mut buf = BytesMut::with_capacity(chunk_size * cnt);
    b.iter(|| {
        let mut salami = Bytes::from(vec![0; chunk_size * cnt]);
        for _ in 0..cnt {
            buf.put(salami.split_to(chunk_size));
        }
        while buf.has_remaining() {
            consume_vectored(&mut buf, BUF_SIZE);
        }
    });
}

#[bench]
fn pass_bytes_through_sliced_by_16_chunked(b: &mut Bencher) {
    pass_bytes_through_chunked(b, BUF_SIZE / 16, 16);
}

#[bench]
fn pass_bytes_through_sliced_by_16_straight(b: &mut Bencher) {
    pass_bytes_through_straight(b, BUF_SIZE / 16, 16);
}

#[bench]
fn pass_bytes_through_sliced_by_64_chunked(b: &mut Bencher) {
    pass_bytes_through_chunked(b, BUF_SIZE / 64, 64);
}

#[bench]
fn pass_bytes_through_sliced_by_64_straight(b: &mut Bencher) {
    pass_bytes_through_straight(b, BUF_SIZE / 64, 64);
}

#[bench]
fn pass_bytes_through_sliced_by_256_chunked(b: &mut Bencher) {
    pass_bytes_through_chunked(b, BUF_SIZE / 256, 256);
}

#[bench]
fn pass_bytes_through_sliced_by_256_straight(b: &mut Bencher) {
    pass_bytes_through_straight(b, BUF_SIZE / 256, 256);
}

#[bench]
fn pass_bytes_through_medium_sized_chunked(b: &mut Bencher) {
    pass_bytes_through_chunked(b, BUF_SIZE / 4, 16);
}

#[bench]
fn pass_bytes_through_medium_sized_straight(b: &mut Bencher) {
    pass_bytes_through_straight(b, BUF_SIZE / 4, 16);
}

#[bench]
fn pass_bytes_through_larger_than_buf_chunked(b: &mut Bencher) {
    pass_bytes_through_chunked(b, BUF_SIZE * 2, 2);
}

#[bench]
fn pass_bytes_through_larger_than_buf_straight(b: &mut Bencher) {
    pass_bytes_through_straight(b, BUF_SIZE * 2, 2);
}
