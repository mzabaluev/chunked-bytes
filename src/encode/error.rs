use std::{
    error::Error,
    fmt::{self, Display},
    io,
};

#[derive(Debug)]
pub enum EncodeError {
    Io(io::Error),
    Unrepresentable(char),
}

impl Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EncodeError::Io(e) => write!(f, "{}", e),
            EncodeError::Unrepresentable(c) => {
                write!(
                    f,
                    "Unicode character U+{:04X} cannot be represented in this encoding",
                    u32::from(*c),
                )
            }
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
