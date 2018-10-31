mod encoder;
mod error;

mod utf8enc;

// Interfaces
pub use self::{encoder::TextEncoder, error::EncodeError};

// Encodrs
pub use self::utf8enc::Utf8Encoder;
