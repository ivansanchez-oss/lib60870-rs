use bytes::{Buf, BufMut};

use crate::error::AduError;
use crate::types::QualityDescriptor;

use super::traits::{Decode, Encode};

/// Measured value, short floating point (IEEE STD 754 + QDS).
///
/// Wire format: 5 bytes
///   - bytes 0-3: f32 value (little-endian)
///   - byte 4: quality descriptor
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredValueShortFloat {
    pub value: f32,
    pub quality: QualityDescriptor,
}

impl MeasuredValueShortFloat {
    pub const ENCODED_SIZE: usize = 5;

    pub fn new(value: f32, quality: QualityDescriptor) -> Self {
        Self { value, quality }
    }
}

impl Encode for MeasuredValueShortFloat {
    fn encode(&self, buf: &mut impl BufMut) -> Result<(), AduError> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(AduError::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        buf.put_f32_le(self.value);
        buf.put_u8(self.quality.bits());
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for MeasuredValueShortFloat {
    fn decode(buf: &mut impl Buf) -> Result<Self, AduError> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(AduError::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let value = buf.get_f32_le();
        let quality = QualityDescriptor::from_bits_truncate(buf.get_u8());
        Ok(Self { value, quality })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn roundtrip() {
        let mv = MeasuredValueShortFloat::new(3.14, QualityDescriptor::OVERFLOW | QualityDescriptor::INVALID);
        let mut buf = BytesMut::with_capacity(16);
        mv.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 5);

        let mut reader = Bytes::from(buf);
        let decoded = MeasuredValueShortFloat::decode(&mut reader).unwrap();
        assert_eq!(mv.value, decoded.value);
        assert_eq!(mv.quality, decoded.quality);
    }

    #[test]
    fn negative_value() {
        let mv = MeasuredValueShortFloat::new(-273.15, QualityDescriptor::empty());
        let mut buf = BytesMut::with_capacity(16);
        mv.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = MeasuredValueShortFloat::decode(&mut reader).unwrap();
        assert!((decoded.value - (-273.15)).abs() < f32::EPSILON);
    }

    #[test]
    fn buffer_too_short() {
        let mut buf = Bytes::from_static(&[0x00, 0x00, 0x00]);
        assert!(MeasuredValueShortFloat::decode(&mut buf).is_err());
    }
}
