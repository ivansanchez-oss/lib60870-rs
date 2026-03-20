use bytes::{Buf, BufMut};

use crate::error::AduError;
use crate::types::QualityDescriptor;

use super::traits::{Decode, Encode};

/// Single-point information (SIQ).
///
/// Wire format: 1 byte
///   - bit 0: SPI (single-point value, true = ON)
///   - bits 4-7: quality descriptor (OV, BL, SB, NT, IV)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SinglePointInformation {
    pub value: bool,
    pub quality: QualityDescriptor,
}

impl SinglePointInformation {
    pub const ENCODED_SIZE: usize = 1;

    pub fn new(value: bool, quality: QualityDescriptor) -> Self {
        Self { value, quality }
    }
}

impl Encode for SinglePointInformation {
    fn encode(&self, buf: &mut impl BufMut) -> Result<(), AduError> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(AduError::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        let byte = (self.quality.bits() & 0xF0) | (self.value as u8);
        buf.put_u8(byte);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for SinglePointInformation {
    fn decode(buf: &mut impl Buf) -> Result<Self, AduError> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(AduError::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let byte = buf.get_u8();
        let value = byte & 0x01 != 0;
        let quality = QualityDescriptor::from_bits_truncate(byte & 0xF0);
        Ok(Self { value, quality })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn roundtrip() {
        let spi = SinglePointInformation::new(true, QualityDescriptor::BLOCKED | QualityDescriptor::INVALID);
        let mut buf = BytesMut::with_capacity(16);
        spi.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 1);

        let mut reader = Bytes::from(buf);
        let decoded = SinglePointInformation::decode(&mut reader).unwrap();
        assert_eq!(spi, decoded);
    }

    #[test]
    fn value_off_good_quality() {
        let spi = SinglePointInformation::new(false, QualityDescriptor::empty());
        let mut buf = BytesMut::with_capacity(16);
        spi.encode(&mut buf).unwrap();
        assert_eq!(buf[0], 0x00);

        let mut reader = Bytes::from(buf);
        let decoded = SinglePointInformation::decode(&mut reader).unwrap();
        assert_eq!(decoded.value, false);
        assert!(decoded.quality.is_empty());
    }

    #[test]
    fn buffer_too_short_encode() {
        let spi = SinglePointInformation::new(true, QualityDescriptor::empty());
        let mut buf = [0u8; 0];
        assert!(spi.encode(&mut buf.as_mut_slice()).is_err());
    }

    #[test]
    fn buffer_too_short_decode() {
        let mut buf = Bytes::new();
        assert!(SinglePointInformation::decode(&mut buf).is_err());
    }
}
