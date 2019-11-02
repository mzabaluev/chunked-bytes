use super::{EncodeError, TextEncoder};
use crate::chunked_bytes::ChunkedBytes;
use range_split::TakeRange;
use strchunk::StrChunk;

pub struct Utf8Encoder {}

impl TextEncoder for Utf8Encoder {
    fn encode(
        &mut self,
        input: &mut StrChunk,
        output: &mut ChunkedBytes,
    ) -> Result<(), EncodeError> {
        let bytes = input.take_range(..).into();
        output.append_chunk(bytes);
        Ok(())
    }
}
