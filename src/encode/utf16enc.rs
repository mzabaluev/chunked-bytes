use super::{EncodeError, TextEncoder};
use bytes::{BufMut, ByteOrder};
use chunked_bytes::ChunkedBytes;
use strchunk::{split::Take, StrChunk};

use std::marker::PhantomData;

pub struct Utf16Encoder<Bo> {
    _byte_order: PhantomData<Bo>,
}

impl<Bo> TextEncoder for Utf16Encoder<Bo>
where
    Bo: ByteOrder,
{
    fn encode(
        &mut self,
        input: &mut StrChunk,
        output: &mut ChunkedBytes,
    ) -> Result<(), EncodeError> {
        // Make sure the output can fit any single complete UTF-16 sequence.
        output.reserve(4);

        let encoded_to = {
            let mut iter = input.char_indices();
            loop {
                if let Some((i, c)) = iter.next() {
                    let mut utf16_buf = [0u16; 2];
                    let utf16_seq = c.encode_utf16(&mut utf16_buf);
                    let bytes_len = utf16_seq.len() * 2;
                    if output.remaining_mut() < bytes_len {
                        // Cannot fit this character into the output buffer
                        // without expanding its capacity.
                        // Return what has been encoded so far,
                        // which should be something.
                        debug_assert!(i != 0);
                        break i;
                    }
                    unsafe {
                        Bo::write_u16_into(utf16_seq, output.bytes_mut());
                        output.advance_mut(bytes_len);
                    }
                } else {
                    break input.len();
                }
            }
        };
        input.remove_range(..encoded_to);
        Ok(())
    }
}
