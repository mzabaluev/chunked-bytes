use super::decode::{DecodeError, TextDecoder};

use bytes::BytesMut;
use futures::{Async, Poll, Stream};
use strchunk::StrChunk;
use tokio_io::AsyncRead;

pub struct TextReader<T, D> {
    reader: T,
    decoder: D,
    buf: BytesMut,
    eof: bool,
}

const DEFAULT_CAPACITY: usize = 8 * 1024;

impl<T, D> TextReader<T, D>
where
    T: AsyncRead,
    D: TextDecoder,
{
    pub fn new(reader: T, decoder: D) -> Self {
        Self::with_capacity(reader, decoder, DEFAULT_CAPACITY)
    }

    pub fn with_capacity(reader: T, decoder: D, capacity: usize) -> Self {
        TextReader {
            reader,
            decoder,
            buf: BytesMut::with_capacity(capacity),
            eof: false,
        }
    }
}

impl<T, D> Stream for TextReader<T, D>
where
    T: AsyncRead,
    D: TextDecoder,
{
    type Item = StrChunk;
    type Error = DecodeError;

    fn poll(&mut self) -> Poll<Option<StrChunk>, DecodeError> {
        loop {
            if self.eof {
                return self.poll_eof();
            }

            // Guard against spurious EOFs by reserving at least one byte
            // to read.
            self.buf.reserve(1);

            let nread = try_ready!(self.reader.read_buf(&mut self.buf));

            if nread == 0 {
                self.eof = true;
            } else {
                let decoded = self.decoder.decode(&mut self.buf)?;
                if let Some(_) = decoded {
                    return Ok(Async::Ready(decoded));
                }
            }
        }
    }
}

impl<T, D> TextReader<T, D>
where
    D: TextDecoder,
{
    fn poll_eof(&mut self) -> Poll<Option<StrChunk>, DecodeError> {
        let decoded = self.decoder.decode_eof(&mut self.buf)?;
        Ok(Async::Ready(decoded))
    }
}
