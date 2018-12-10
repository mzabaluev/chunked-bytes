#[macro_use]
extern crate futures;
extern crate tokio_io;

extern crate bytes;
extern crate iovec;
extern crate strchunk;

pub mod chunked_bytes;
pub mod decode;
pub mod encode;

mod read;
mod write;

pub use crate::read::TextReader;
pub use crate::write::TextWriter;
