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
//! access to the buffered data. It also provides the `put_bytes` method
//! for appending a `Bytes` slice to its queue of non-contiguous chunks without
//! copying the data.
//!
//! # Examples
//!
//! ```
//! use bytes::{Buf, BufMut, Bytes};
//! use chunked_bytes::ChunkedBytes;
//! use std::io::{self, IoSlice, Read, Write};
//! use std::net::{SocketAddr, TcpListener, TcpStream, Shutdown};
//! use std::thread;
//!
//! fn write_vectored<W: Write>(
//!     buf: &mut ChunkedBytes,
//!     mut out: W,
//! ) -> io::Result<usize> {
//!     let mut io_bufs = [IoSlice::new(&[]); 32];
//!     let io_vec_len = buf.bytes_vectored(&mut io_bufs);
//!     let bytes_written = out.write_vectored(&io_bufs[..io_vec_len])?;
//!     buf.advance(bytes_written);
//!     Ok(bytes_written)
//! }
//!
//! fn main() -> io::Result<()> {
//!     const MESSAGE: &[u8] = b"I \xf0\x9f\x96\xa4 \x00\xc0\xff\xee";
//!
//!     let listen_addr = "127.0.0.1:0".parse::<SocketAddr>().unwrap();
//!     let server = TcpListener::bind(listen_addr)?;
//!     let server_addr = server.local_addr()?;
//!
//!     let server_handle: thread::JoinHandle<io::Result<()>> =
//!         thread::spawn(move || {
//!             let (mut receiver, _) = server.accept()?;
//!             let mut buf = Vec::with_capacity(64);
//!             receiver.read_to_end(&mut buf)?;
//!             assert_eq!(buf.as_slice(), MESSAGE);
//!             Ok(())
//!         });
//!
//!     let mut sender = TcpStream::connect(server_addr)?;
//!
//!     let mut buf = ChunkedBytes::with_chunk_size_hint(4096);
//!
//!     buf.put("I ".as_bytes());
//!     buf.put_bytes(Bytes::from("🖤 "));
//!     buf.put_u32(0xc0ffee);
//!
//!     let bytes_written = write_vectored(&mut buf, &mut sender)?;
//!     assert_eq!(bytes_written, MESSAGE.len());
//!
//!     sender.shutdown(Shutdown::Write)?;
//!
//!     server_handle.join().expect("server thread panicked")?;
//!     Ok(())
//! }

#![warn(clippy::all)]
#![warn(future_incompatible)]
#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![doc(test(no_crate_inject, attr(deny(warnings, rust_2018_idioms))))]

pub mod loosely;
pub mod strictly;

mod chunked;
mod iter;

pub use self::iter::{DrainChunks, IntoChunks};
pub use self::loosely::ChunkedBytes;

#[cfg(test)]
mod tests;
