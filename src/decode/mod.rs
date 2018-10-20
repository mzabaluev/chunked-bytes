mod decoder;
pub use self::decoder::TextDecoder;

mod error;
pub use self::error::{DecodeError, RecoveryInfo};

mod utf8dec;
pub use self::utf8dec::Utf8Decoder;

mod utf16dec;
pub use self::utf16dec::Utf16Decoder;
