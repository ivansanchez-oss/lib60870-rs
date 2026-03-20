use bytes::{Buf, BufMut};

use crate::error::AduError;

/// Trait for encoding a value into a byte buffer.
pub trait Encode {
    fn encode(&self, buf: &mut impl BufMut) -> Result<(), AduError>;
    fn encoded_size(&self) -> usize;
}

/// Trait for decoding a value from a byte buffer.
pub trait Decode: Sized {
    fn decode(buf: &mut impl Buf) -> Result<Self, AduError>;
}
