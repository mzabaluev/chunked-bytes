use std::{
    error::Error,
    fmt::{self, Display},
    io,
};

#[derive(Debug)]
pub enum EncodeError {
    Unrepresentable(char),
    Io(io::Error),
}

impl Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EncodeError::Unrepresentable(c) => {
                write!(f, "unrepresentable Unicode character '{}' in input", c)
            }
            EncodeError::Io(io_err) => write!(f, "{}", io_err),
        }
    }
}

impl Error for EncodeError {
    fn cause(&self) -> Option<&Error> {
        if let EncodeError::Io(ref io_err) = *self {
            Some(io_err)
        } else {
            None
        }
    }
}

impl From<io::Error> for EncodeError {
    fn from(src: io::Error) -> EncodeError {
        EncodeError::Io(src)
    }
}
