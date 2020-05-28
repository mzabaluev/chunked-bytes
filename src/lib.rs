#![warn(clippy::all)]
#![warn(future_incompatible)]
#![warn(rust_2018_idioms)]

mod chunked_bytes;
mod iter;

pub use chunked_bytes::ChunkedBytes;
pub use iter::{DrainChunks, IntoChunks};
