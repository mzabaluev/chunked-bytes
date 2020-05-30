// This example demonstrates the primary use case for `ChunkedBytes`:
// a buffer to serialize data of protocol messages without reallocations
// and write it to output with efficiency of `AsyncWrite` implementations
// that make use of `Buf::bytes_vectored`.

use bytes::{Buf, BufMut, Bytes, BytesMut};
use chunked_bytes::ChunkedBytes;
use futures::prelude::*;
use futures::ready;
use futures::Sink;
use pin_project::pin_project;
use tokio::io::{self, AsyncWrite, Error};
use tokio::prelude::*;

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
    buf.push_chunk(msg.blob_field);
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

    fn poll_flush_buf(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        let mut this = self.project();
        while this.buf.has_remaining() {
            ready!(this.out.as_mut().poll_write_buf(cx, this.buf))?;
        }
        Poll::Ready(Ok(()))
    }
}

impl<T: AsyncWrite> Sink<Message> for EncodingWriter<T> {
    type Error = Error;

    fn poll_ready(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>> {
        let mut this = self.project();
        // Here's a way to provide back-pressure on the sink:
        // rather than allowing the buffer to grow, drain it until
        // there is only the staging buffer with room to fill.
        while this.buf.remaining() >= this.buf.preferred_chunk_size() {
            ready!(this.out.as_mut().poll_write_buf(cx, this.buf))?;
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
        self.project().out.poll_shutdown(cx)
    }
}

#[tokio::main]
async fn main() {
    // Pretend we received the data from input into a Bytes handle
    let mut blob = BytesMut::with_capacity(8000);
    io::repeat(b'\xa5').read_buf(&mut blob).await.unwrap();

    let msg = Message {
        int_field: 42,
        string_field: "Hello, world!".into(),
        blob_field: blob.freeze(),
    };

    let sink = io::sink();
    let mut writer = EncodingWriter::new(sink);

    writer.send(msg).await.unwrap();
}
