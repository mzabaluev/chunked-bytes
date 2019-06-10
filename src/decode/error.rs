use std::{
    error::Error,
    fmt::{self, Display},
    io,
};
use strchunk::{ExtractUtf8Error, StrChunk};

#[derive(Debug)]
pub enum DecodeError {
    Encoding(RecoveryInfo),
    Io(io::Error),
}

#[derive(Debug)]
pub struct RecoveryInfo {
    decoded: StrChunk,
    skip_len: Option<usize>,
}

impl DecodeError {
    pub fn incomplete(decoded: StrChunk) -> Self {
        DecodeError::Encoding(RecoveryInfo {
            decoded,
            skip_len: None,
        })
    }

    pub fn with_recovery(decoded: StrChunk, skip_len: usize) -> Self {
        DecodeError::Encoding(RecoveryInfo {
            decoded,
            skip_len: Some(skip_len),
        })
    }
}

impl RecoveryInfo {
    pub fn skip_len(&self) -> Option<usize> {
        self.skip_len
    }

    pub fn into_decoded(self) -> StrChunk {
        self.decoded
    }
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecodeError::Encoding(RecoveryInfo { skip_len, .. }) => {
                match skip_len {
                    Some(_) => write!(f, "invalid encoding sequence in input"),
                    None => write!(f, "incomplete encoding input"),
                }
            }
            DecodeError::Io(io_err) => write!(f, "{}", io_err),
        }
    }
}

impl Error for DecodeError {
    fn cause(&self) -> Option<&Error> {
        if let DecodeError::Io(ref io_err) = *self {
            Some(io_err)
        } else {
            None
        }
    }
}

impl From<io::Error> for DecodeError {
    fn from(src: io::Error) -> DecodeError {
        DecodeError::Io(src)
    }
}

impl From<ExtractUtf8Error> for DecodeError {
    fn from(src: ExtractUtf8Error) -> DecodeError {
        let skip_len = src.error_len();
        let decoded = src.into_extracted();
        DecodeError::with_recovery(decoded, skip_len)
    }
}
