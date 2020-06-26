# Chunked Bytes

This crate provides two variants of `ChunkedBytes`, a non-contiguous buffer
for efficient data structure serialization and vectored output.

In network programming, there is often a need to serialize data structures
into a wire protocol representation and send the resulting sequence of bytes
to an output object, such as a socket. For a variety of reasons, developers
of protocol implementations prefer to serialize the data into
an intermediate buffer in memory rather than deal with output objects directly
in either synchronous or asynchronous form, or both. When a contiguous
buffer like `Vec` is used for this, reallocations to fit a larger length of
serialized data may adversely impact performance, while evaluating the required
length to pre-allocate beforehand may be cumbersome or difficult in the
context. The single contiguous buffer also forms a boundary for write requests,
creating a need for copy-back schemes to avoid inefficiently fragmented writes
of tail data.

Another important use case is passing network data through. If some of the data
is received into [`Bytes`](https://docs.rs/bytes) handles, it should be possible
to inject the data into the output stream without extra copying.

Enter `ChunkedBytes`, containers that can be used to coalesce data added as
byte slices via the `BufMut` interface, as well as possible `Bytes` input,
into a sequence of chunks suitable for implementations of the
[`Write::write_vectored`][write_vectored] method. This design aims to deliver
good performance regardless of the size of buffered data and with no need
to pre-allocate sufficient capacity for it.

[write_vectored]: https://doc.rust-lang.org/stable/std/io/trait.Write.html#method.write_vectored

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `chunked-bytes` by you, shall be licensed as MIT, without any
additional terms or conditions.
