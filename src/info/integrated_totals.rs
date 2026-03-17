use bytes::{Buf, BufMut};

use crate::error::{Error, Result};

use super::traits::{Decode, Encode};

/// Binary counter reading (BCR).
///
/// Wire format: 5 bytes
///   - bytes 0-3: counter value (i32, little-endian)
///   - byte 4: sequence number (bits 0-4), carry (bit 5),
///     counter adjusted (bit 6), invalid (bit 7)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryCounterReading {
    pub value: i32,
    pub sequence_number: u8,
    pub carry: bool,
    pub adjusted: bool,
    pub invalid: bool,
}

impl BinaryCounterReading {
    pub const ENCODED_SIZE: usize = 5;

    pub fn new(value: i32, sequence_number: u8) -> Self {
        Self {
            value,
            sequence_number: sequence_number & 0x1F,
            carry: false,
            adjusted: false,
            invalid: false,
        }
    }
}

impl Encode for BinaryCounterReading {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        buf.put_i32_le(self.value);
        let flags = (self.sequence_number & 0x1F)
            | ((self.carry as u8) << 5)
            | ((self.adjusted as u8) << 6)
            | ((self.invalid as u8) << 7);
        buf.put_u8(flags);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for BinaryCounterReading {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let value = buf.get_i32_le();
        let flags = buf.get_u8();
        Ok(Self {
            value,
            sequence_number: flags & 0x1F,
            carry: flags & 0x20 != 0,
            adjusted: flags & 0x40 != 0,
            invalid: flags & 0x80 != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn roundtrip() {
        let mut bcr = BinaryCounterReading::new(123456, 15);
        bcr.carry = true;
        bcr.invalid = true;

        let mut buf = BytesMut::with_capacity(16);
        bcr.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 5);

        let mut reader = Bytes::from(buf);
        let decoded = BinaryCounterReading::decode(&mut reader).unwrap();
        assert_eq!(bcr, decoded);
    }

    #[test]
    fn negative_value() {
        let bcr = BinaryCounterReading::new(-999999, 0);
        let mut buf = BytesMut::with_capacity(16);
        bcr.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = BinaryCounterReading::decode(&mut reader).unwrap();
        assert_eq!(decoded.value, -999999);
    }

    #[test]
    fn all_flags() {
        let mut bcr = BinaryCounterReading::new(0, 31);
        bcr.carry = true;
        bcr.adjusted = true;
        bcr.invalid = true;

        let mut buf = BytesMut::with_capacity(16);
        bcr.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = BinaryCounterReading::decode(&mut reader).unwrap();
        assert_eq!(decoded.sequence_number, 31);
        assert!(decoded.carry);
        assert!(decoded.adjusted);
        assert!(decoded.invalid);
    }

    #[test]
    fn buffer_too_short() {
        let mut buf = Bytes::from_static(&[0x00, 0x00]);
        assert!(BinaryCounterReading::decode(&mut buf).is_err());
    }
}
