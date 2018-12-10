use crate::chunked_bytes::ChunkedBytes;
use crate::encode::{EncodeError, TextEncoder};

use bytes::Buf;
use futures::prelude::*;
use strchunk::StrChunk;
use tokio_io::AsyncWrite;

pub struct TextWriter<T, E> {
    writer: T,
    encoder: E,
    state: WriterState,
}

struct WriterState {
    remaining_str: StrChunk,
    encoded_buf: ChunkedBytes,
    eof_encoded: bool,
}

impl<T, E> Sink for TextWriter<T, E>
where
    T: AsyncWrite,
    E: TextEncoder,
{
    type SinkItem = StrChunk;
    type SinkError = EncodeError;

    fn start_send(
        &mut self,
        item: StrChunk,
    ) -> StartSend<StrChunk, EncodeError> {
        let async_sink = self.state.start(item);
        if async_sink.is_not_ready() {
            Ok(async_sink)
        } else {
            self.encoder.encode(
                &mut self.state.remaining_str,
                &mut self.state.encoded_buf,
            )?;
            self.writer.write_buf(&mut self.state.encoded_buf)?;
            Ok(AsyncSink::Ready)
        }
    }

    fn poll_complete(&mut self) -> Poll<(), EncodeError> {
        loop {
            try_ready!(self.encode_more());

            if self.state.try_end() {
                // We're done.
                return Ok(Async::Ready(()));
            } else {
                // Drive writing the output buffer until not ready.
                try_ready!(self.writer.write_buf(&mut self.state.encoded_buf));
            }
        }
    }

    fn close(&mut self) -> Poll<(), EncodeError> {
        loop {
            try_ready!(self.encode_more());

            // If there is nothing left to encode, finalize the encoder
            // state. This may lead to more data written into the buffer.
            // Optimistically, don't try to drain the buffer beforehand,
            // as there normally should be little if any bytes written
            // and the buffer will absorb it with extra allocation
            // in the worst case.
            if self.state.remaining_str.is_empty() && !self.state.eof_encoded {
                self.encoder.encode_eof(&mut self.state.encoded_buf)?;
                self.state.eof_encoded = true;
            }

            if self.state.try_end() {
                // We're done.
                return Ok(Async::Ready(()));
            } else {
                // Drive writing the output buffer until not ready.
                try_ready!(self.writer.write_buf(&mut self.state.encoded_buf));
            }
        }
    }
}

impl<T, E> TextWriter<T, E>
where
    T: AsyncWrite,
    E: TextEncoder,
{
    fn encode_more(&mut self) -> Poll<(), EncodeError> {
        // First try to write the output buffer until its length is
        // below the nominal chunk size. If not ready to write,
        // bail out as not ready without doing any further encoding.
        while self.state.buf_filled() {
            try_ready!(self.writer.write_buf(&mut self.state.encoded_buf));
        }

        // Then encode the next piece of the string,
        // if there is something left of it.
        if !self.state.remaining_str.is_empty() {
            debug_assert!(!self.state.eof_encoded);
            self.encoder.encode(
                &mut self.state.remaining_str,
                &mut self.state.encoded_buf,
            )?;
        }

        Ok(Async::Ready(()))
    }
}

impl WriterState {
    fn start(&mut self, item: StrChunk) -> AsyncSink<StrChunk> {
        assert!(!self.eof_encoded, "start_send is called after close");
        if !self.remaining_str.is_empty() {
            return AsyncSink::NotReady(item);
        }
        self.remaining_str = item;
        AsyncSink::Ready
    }

    fn try_end(&mut self) -> bool {
        // Sending is complete when the entire string has been encoded
        // and the output buffer has been written.
        if self.encoded_buf.is_empty() {
            // Catch badly written encoders that fail to exhaust the
            // input string while not reporting errors.
            assert!(self.remaining_str.is_empty());
            // Make sure the string buffer passed to start_send is released
            self.remaining_str = StrChunk::new();
            true
        } else {
            false
        }
    }

    fn buf_filled(&self) -> bool {
        let buf = &self.encoded_buf;
        buf.remaining() >= buf.chunk_size()
    }
}
