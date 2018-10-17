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
    decoded: Option<StrChunk>,
    error_len: Option<usize>,
}

impl DecodeError {
    pub fn incomplete(decoded: Option<StrChunk>) -> Self {
        DecodeError::Encoding(RecoveryInfo {
            decoded,
            error_len: None,
        })
    }

    pub fn with_recovery(decoded: Option<StrChunk>, error_len: usize) -> Self {
        DecodeError::Encoding(RecoveryInfo {
            decoded,
            error_len: Some(error_len),
        })
    }
}

impl RecoveryInfo {
    pub fn error_len(&self) -> Option<usize> {
        self.error_len
    }

    pub fn into_decoded(self) -> Option<StrChunk> {
        self.decoded
    }
}

impl Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DecodeError::Encoding(RecoveryInfo {
                error_len: Some(_), ..
            }) => write!(f, "invalid encoding sequence in input"),

            DecodeError::Encoding(RecoveryInfo {
                error_len: None, ..
            }) => write!(f, "incomplete encoding input"),

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
        let error_len = src.error_len();
        let decoded = src.into_extracted();
        DecodeError::with_recovery(decoded, error_len)
    }
}
