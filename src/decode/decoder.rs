use super::DecodeError;

use bytes::BytesMut;
use strchunk::StrChunk;

pub trait TextDecoder {
    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<StrChunk>, DecodeError>;

    fn decode_eof(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<StrChunk>, DecodeError> {
        let decoded = self.decode(src)?;
        if src.is_empty() {
            Ok(decoded)
        } else {
            Err(DecodeError::incomplete(decoded.unwrap_or_default()))
        }
    }
}
