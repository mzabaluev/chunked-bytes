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
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            EncodeError::Io(e) => Some(e),
            EncodeError::Unrepresentable(_) => None,
        }
    }
}

impl From<io::Error> for EncodeError {
    fn from(src: io::Error) -> EncodeError {
        EncodeError::Io(src)
    }
}
