use super::{DecodeError, TextDecoder};

use bytes::{ByteOrder, BytesMut};
use strchunk::{split::Take, StrChunk, StrChunkMut};

use std::{char, marker::PhantomData};

pub struct Utf16Decoder<Bo> {
    _byte_order: PhantomData<Bo>,
    state: DecoderState,
}

struct DecoderState {
    lead_surrogate: Option<u16>,
    buf: StrChunkMut,
}

enum StepOutcome {
    Continue,
    Break,
}

impl DecoderState {
    fn take_decoded(&mut self) -> StrChunk {
        self.buf.take_range(..).freeze()
    }

    fn decode_step(
        &mut self,
        code_unit: u16,
    ) -> Result<StepOutcome, DecodeError> {
        use self::StepOutcome::*;

        let lead_surrogate = self.lead_surrogate.take();
        let c = match lead_surrogate {
            None => match code_unit {
                0xD800..=0xDBFF => {
                    self.lead_surrogate = Some(code_unit);
                    return Ok(Continue);
                }
                0xDC00..=0xDFFF => {
                    let decoded = self.take_decoded();
                    return Err(DecodeError::with_recovery(decoded, 2));
                }
                _ => unsafe { char::from_u32_unchecked(code_unit as u32) },
            },
            Some(hs) => match code_unit {
                0xDC00..=0xDFFF => {
                    let cp = 0x10000
                        + (((hs & 0x3FF) as u32) << 10)
                        + ((code_unit & 0x3FF) as u32);
                    unsafe { char::from_u32_unchecked(cp) }
                }
                _ => {
                    let decoded = self.take_decoded();
                    return Err(DecodeError::with_recovery(decoded, 0));
                }
            },
        };
        if self.buf.remaining_mut() < c.len_utf8() {
            // We can decode a complete character with the input
            // ahead, but there is no space for it in the output.
            // Leave decoding in this state, to repeat this step next time.
            self.lead_surrogate = lead_surrogate;
            return Ok(Break);
        }
        self.buf.put_char(c);
        Ok(Continue)
    }
}

impl<Bo> TextDecoder for Utf16Decoder<Bo>
where
    Bo: ByteOrder,
{
    fn decode(&mut self, src: &mut BytesMut) -> Result<StrChunk, DecodeError> {
        use self::StepOutcome::*;

        debug_assert!(
            self.state.buf.is_empty(),
            "the output buffer is not empty"
        );
        debug_assert!(
            self.state.buf.capacity() >= 4,
            "the output buffer is too small"
        );

        while src.len() >= 2 {
            let code_unit = Bo::read_u16(src);
            match self.state.decode_step(code_unit)? {
                Break => {
                    break;
                }
                Continue => {
                    src.advance(2);
                }
            }
        }
        Ok(self.state.take_decoded())
    }
}
