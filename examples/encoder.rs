// This example demonstrates the primary use case for `ChunkedBytes`:
// a buffer to serialize data of protocol messages without reallocations
// and write it to output with efficiency of `AsyncWrite` implementations
// that make use of `Buf::chunks_vectored`.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use chunked_bytes::ChunkedBytes;
use futures::io::{self, AsyncWrite, Error, IoSlice};
use futures::prelude::*;
use futures::ready;
use futures::Sink;
use pin_project::pin_project;

use std::iter::{self, FromIterator};
use std::pin::Pin;
use std::task::{Context, Poll};

const MESSAGE_MAGIC: &[u8] = b"mess";

pub struct Message {
    int_field: u32,
    string_field: String,
    blob_field: Bytes,
}

fn encode_message(buf: &mut ChunkedBytes, msg: Message) {
    buf.put_slice(MESSAGE_MAGIC);

    buf.put_u32(1);
    buf.put_u32(msg.int_field);

    buf.put_u32(2);
    buf.put_u64(msg.string_field.len() as u64);
    buf.put(msg.string_field.as_bytes());

    buf.put_u32(3);
    buf.put_u64(msg.blob_field.len() as u64);
    // Take the bytes without copying
    buf.put_bytes(msg.blob_field);
}

#[pin_project]
pub struct EncodingWriter<T> {
    #[pin]
    out: T,
    buf: ChunkedBytes,
}

impl<T: AsyncWrite> EncodingWriter<T> {
    pub fn new(out: T) -> Self {
        EncodingWriter {
            buf: ChunkedBytes::new(),
            out,
        }
    }

    fn poll_write_buf(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        let mut io_bufs = [IoSlice::new(&[]); 16];
        let mut this = self.project();
        let io_vec_len = this.buf.chunks_vectored(&mut io_bufs);
        let bytes_written = ready!(this
            .out
            .as_mut()
            .poll_write_vectored(cx, &io_bufs[..io_vec_len]))?;
        this.buf.advance(bytes_written);
        Poll::Ready(Ok(()))
    }

    fn poll_flush_buf(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        while self.as_mut().project().buf.has_remaining() {
            ready!(self.as_mut().poll_write_buf(cx))?;
        }
        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncWrite> Sink<Message> for EncodingWriter<T> {
    type Error = Error;

    fn poll_ready(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        // Here's a way to provide back-pressure on the sink:
        // rather than allowing the buffer to grow, drain it until
        // there is only the staging buffer with room to fill.
        let chunk_size = self.buf.chunk_size_hint();
        while self.as_mut().buf.remaining() >= chunk_size {
            ready!(self.as_mut().poll_write_buf(cx))?;
        }
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, msg: Message) -> Result<(), Error> {
        let this = self.project();
        encode_message(this.buf, msg);
        Ok(())
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        ready!(self.as_mut().poll_flush_buf(cx))?;
        self.project().out.poll_flush(cx)
    }

    fn poll_close(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        ready!(self.as_mut().poll_flush_buf(cx))?;
        self.project().out.poll_close(cx)
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    // Pretend we received the data from input into a Bytes handle
    let blob = BytesMut::from_iter(iter::repeat(b'\xa5').take(8000));

    let msg = Message {
        int_field: 42,
        string_field: "Hello, world!".into(),
        blob_field: blob.freeze(),
    };

    let sink = io::sink();
    let mut writer = EncodingWriter::new(sink);

    writer.send(msg).await?;
    Ok(())
}
