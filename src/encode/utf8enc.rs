use super::{EncodeError, TextEncoder};
use bytes::Bytes;
use strchunk::{split::Take, StrChunk};

pub struct Utf8Encoder {}

impl TextEncoder for Utf8Encoder {
    fn encode(&mut self, input: &mut StrChunk) -> Result<Bytes, EncodeError> {
        Ok(Bytes::from(input.take(..)))
    }
}
