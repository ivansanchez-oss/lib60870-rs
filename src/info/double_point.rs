use bytes::{Buf, BufMut};

use crate::error::{Error, Result};
use crate::types::{DoublePointValue, QualityDescriptor};

use super::traits::{Decode, Encode};

/// Double-point information (DIQ).
///
/// Wire format: 1 byte
///   - bits 0-1: DPI (double-point value)
///   - bits 4-7: quality descriptor (BL, SB, NT, IV)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoublePointInformation {
    pub value: DoublePointValue,
    pub quality: QualityDescriptor,
}

impl DoublePointInformation {
    pub const ENCODED_SIZE: usize = 1;

    pub fn new(value: DoublePointValue, quality: QualityDescriptor) -> Self {
        Self { value, quality }
    }
}

impl Encode for DoublePointInformation {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        let byte = (self.quality.bits() & 0xF0) | (self.value as u8 & 0x03);
        buf.put_u8(byte);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for DoublePointInformation {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let byte = buf.get_u8();
        let value = DoublePointValue::from_raw(byte);
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
        let dpi = DoublePointInformation::new(DoublePointValue::On, QualityDescriptor::SUBSTITUTED);
        let mut buf = BytesMut::with_capacity(16);
        dpi.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 1);

        let mut reader = Bytes::from(buf);
        let decoded = DoublePointInformation::decode(&mut reader).unwrap();
        assert_eq!(dpi, decoded);
    }

    #[test]
    fn all_values() {
        for raw in 0..=3u8 {
            let val = DoublePointValue::from_raw(raw);
            let dpi = DoublePointInformation::new(val, QualityDescriptor::empty());
            let mut buf = BytesMut::with_capacity(16);
            dpi.encode(&mut buf).unwrap();
            let mut reader = Bytes::from(buf);
            let decoded = DoublePointInformation::decode(&mut reader).unwrap();
            assert_eq!(decoded.value, val);
        }
    }

    #[test]
    fn buffer_too_short() {
        let mut buf = Bytes::new();
        assert!(DoublePointInformation::decode(&mut buf).is_err());
    }
}
