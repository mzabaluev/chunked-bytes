use super::EncodeError;
use chunked_bytes::ChunkedBytes;

use strchunk::StrChunk;

pub trait TextEncoder {
    fn encode(
        &mut self,
        input: &mut StrChunk,
        output: &mut ChunkedBytes,
    ) -> Result<(), EncodeError>;

    fn encode_eof(
        &mut self,
        _output: &mut ChunkedBytes,
    ) -> Result<(), EncodeError> {
        Ok(())
    }
}
