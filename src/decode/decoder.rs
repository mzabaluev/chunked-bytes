use super::DecodeError;

use bytes::BytesMut;
use strchunk::StrChunk;

pub trait TextDecoder {
    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<StrChunk, DecodeError>;

    fn decode_eof(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<StrChunk, DecodeError> {
        let decoded = self.decode(src)?;
        if src.is_empty() {
            Ok(decoded)
        } else {
            Err(DecodeError::incomplete_input(decoded))
        }
    }
}
