use super::{DecodeError, TextDecoder};

use bytes::BytesMut;
use strchunk::StrChunk;

pub struct Utf8Decoder;

impl Utf8Decoder {
    pub fn new() -> Utf8Decoder {
        Utf8Decoder
    }
}

impl TextDecoder for Utf8Decoder {
    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<StrChunk, DecodeError> {
        StrChunk::extract_utf8(src).map_err(|e| e.into())
    }
}
