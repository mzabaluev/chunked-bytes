use super::EncodeError;

use bytes::Bytes;
use strchunk::StrChunk;

pub trait TextEncoder {
    fn encode(&mut self, input: &mut StrChunk) -> Result<Bytes, EncodeError>;

    fn encode_eof(&mut self) -> Result<Bytes, EncodeError> {
        Ok(Bytes::new())
    }
}
