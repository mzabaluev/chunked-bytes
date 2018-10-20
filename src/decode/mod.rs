mod decoder;
mod error;

mod utf16dec;
mod utf8dec;

// Interfaces
pub use self::{
    decoder::TextDecoder,
    error::{DecodeError, RecoveryInfo},
};

// Decoders
pub use self::utf16dec::Utf16Decoder;
pub use self::utf8dec::Utf8Decoder;
