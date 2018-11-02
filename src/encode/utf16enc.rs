use super::{EncodeError, TextEncoder};
use bytes::{BufMut, ByteOrder, Bytes, BytesMut};
use strchunk::{split::Take, StrChunk};

use std::marker::PhantomData;

pub struct Utf16Encoder<Bo> {
    _byte_order: PhantomData<Bo>,
    buf: BytesMut,
}

impl<Bo> TextEncoder for Utf16Encoder<Bo>
where
    Bo: ByteOrder,
{
    fn encode(&mut self, input: &mut StrChunk) -> Result<Bytes, EncodeError> {
        debug_assert!(self.buf.is_empty() && self.buf.remaining_mut() >= 4);
        let encoded_to = {
            let mut iter = input.char_indices();
            loop {
                if let Some((i, c)) = iter.next() {
                    let mut utf16_buf = [0u16; 2];
                    let utf16_seq = c.encode_utf16(&mut utf16_buf);
                    let bytes_len = utf16_seq.len() * 2;
                    if self.buf.remaining_mut() < bytes_len {
                        // Cannot fit this character into the output buffer.
                        // Return what has been encoded so far,
                        // which should be something.
                        debug_assert!(i != 0 && !self.buf.is_empty());
                        break i;
                    }
                    unsafe {
                        Bo::write_u16_into(utf16_seq, self.buf.bytes_mut());
                        self.buf.advance_mut(bytes_len);
                    }
                } else {
                    break input.len();
                }
            }
        };
        input.remove_range(..encoded_to);
        Ok(self.buf.take().freeze())
    }
}
