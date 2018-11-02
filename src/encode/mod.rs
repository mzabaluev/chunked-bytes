mod encoder;
mod error;

mod utf16enc;
mod utf8enc;

// Interfaces
pub use self::{encoder::TextEncoder, error::EncodeError};

// Encoders
pub use self::utf16enc::Utf16Encoder;
pub use self::utf8enc::Utf8Encoder;
