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

impl DecoderState {
    fn commit(&mut self, lead_surrogate: Option<u16>) -> Option<StrChunk> {
        self.lead_surrogate = lead_surrogate;
        if self.buf.is_empty() {
            None
        } else {
            Some(self.buf.take(..).freeze())
        }
    }
}

enum StepResult {
    Char(char),
    HighSurrogate(u16),
    Error(Recovery),
}

enum Recovery {
    Keep,
    Skip,
}

fn decode_step(lead_surrogate: Option<u16>, code_unit: u16) -> StepResult {
    match lead_surrogate {
        None => match code_unit {
            0xD800..=0xDBFF => StepResult::HighSurrogate(code_unit),
            0xDC00..=0xDFFF => StepResult::Error(Recovery::Skip),
            _ => StepResult::Char(unsafe {
                char::from_u32_unchecked(code_unit.into())
            }),
        },
        Some(hs) => match code_unit {
            0xDC00..=0xDFFF => {
                let cp: u32 = 0x10000
                    + (((hs & 0x3FF) as u32) << 10)
                    + ((code_unit & 0x3FF) as u32);
                StepResult::Char(unsafe { char::from_u32_unchecked(cp) })
            }
            _ => StepResult::Error(Recovery::Keep),
        },
    }
}

impl<Bo> TextDecoder for Utf16Decoder<Bo>
where
    Bo: ByteOrder,
{
    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> Result<Option<StrChunk>, DecodeError> {
        debug_assert!({
            let buf = &self.state.buf;
            buf.is_empty() && buf.capacity() >= 4
        });
        let mut lead_surrogate = self.state.lead_surrogate;
        while src.len() >= 2 {
            let step_res = decode_step(lead_surrogate, Bo::read_u16(src));
            lead_surrogate = match step_res {
                StepResult::Char(c) => {
                    let buf = &mut self.state.buf;
                    if buf.remaining_mut() < c.len_utf8() {
                        // We can decode a complete character with the input
                        // ahead, but there is no space for it in the output.
                        // Leave decoding in this state, to repeat the last
                        // step next time.
                        break;
                    }
                    buf.put_char(c);
                    None
                }
                StepResult::HighSurrogate(hs) => Some(hs),
                StepResult::Error(recovery) => {
                    let decoded = self.state.commit(None);
                    let skip_len = match recovery {
                        Recovery::Keep => 0,
                        Recovery::Skip => 2,
                    };
                    return Err(DecodeError::with_recovery(decoded, skip_len));
                }
            };
            src.advance(2);
        }
        Ok(self.state.commit(lead_surrogate))
    }
}
