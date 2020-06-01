//! A non-contiguous buffer for efficient serialization of data structures
//! and vectored output.
//!
//! This crate provides `ChunkedBytes`, a [rope]-like byte container based on
//! `Bytes` and `BytesMut` from the `bytes` crate. Its primary purpose is to
//! serve as an intermediate buffer for serializing fields of data structures
//! into byte sequences of varying length, without whole-buffer reallocations
//! like those performed to grow a `Vec`, and then consuming the bytes in bulk,
//! split into regularly sized chunks suitable for [vectored output].
//!
//! [rope]: https://en.wikipedia.org/wiki/Rope_(data_structure)
//! [vectored output]: https://en.wikipedia.org/wiki/Vectored_I/O
//!
//! `ChunkedBytes` implements the traits `Buf` and `BufMut` for read and write
//! access to the buffered data. It also provides the `push_chunk` method
//! for appending a `Bytes` slice to its queue of non-contiguous chunks without
//! copying the data.

#![warn(clippy::all)]
#![warn(future_incompatible)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

mod chunked_bytes;
mod iter;

pub use chunked_bytes::ChunkedBytes;
pub use iter::{DrainChunks, IntoChunks};
