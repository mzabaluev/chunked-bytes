use super::DecodeError;

use bytes::BytesMut;
use strchunk::StrChunk;
use tokio_codec::Decoder;

pub struct Utf8Decoder {}

impl Decoder for Utf8Decoder {
    type Item = StrChunk;
    type Error = DecodeError;

    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<StrChunk>, Self::Error> {
        let decoded = StrChunk::extract_utf8(src)?;
        Ok(decoded)
    }

    fn decode_eof(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<StrChunk>, Self::Error> {
        let decoded = StrChunk::extract_utf8(src)?;
        if src.is_empty() {
            Ok(decoded)
        } else {
            Err(DecodeError::incomplete(decoded))
        }
    }
}
