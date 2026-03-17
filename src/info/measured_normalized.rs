use bytes::{Buf, BufMut};

use crate::error::{Error, Result};
use crate::types::QualityDescriptor;

use super::traits::{Decode, Encode};

/// Measured value, normalized (NVA + QDS).
///
/// Wire format: 3 bytes
///   - bytes 0-1: normalized value (i16, little-endian)
///   - byte 2: quality descriptor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasuredValueNormalized {
    pub value: i16,
    pub quality: QualityDescriptor,
}

impl MeasuredValueNormalized {
    pub const ENCODED_SIZE: usize = 3;

    pub fn new(value: i16, quality: QualityDescriptor) -> Self {
        Self { value, quality }
    }
}

impl Encode for MeasuredValueNormalized {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        buf.put_i16_le(self.value);
        buf.put_u8(self.quality.bits());
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for MeasuredValueNormalized {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let value = buf.get_i16_le();
        let quality = QualityDescriptor::from_bits_truncate(buf.get_u8());
        Ok(Self { value, quality })
    }
}

/// Measured value, normalized without quality descriptor (for M_ME_ND_1).
///
/// Wire format: 2 bytes (i16, little-endian)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasuredValueNormalizedNoQuality {
    pub value: i16,
}

impl MeasuredValueNormalizedNoQuality {
    pub const ENCODED_SIZE: usize = 2;

    pub fn new(value: i16) -> Self {
        Self { value }
    }
}

impl Encode for MeasuredValueNormalizedNoQuality {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        buf.put_i16_le(self.value);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for MeasuredValueNormalizedNoQuality {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let value = buf.get_i16_le();
        Ok(Self { value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn normalized_roundtrip() {
        let mv = MeasuredValueNormalized::new(-12345, QualityDescriptor::OVERFLOW);
        let mut buf = BytesMut::with_capacity(16);
        mv.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 3);

        let mut reader = Bytes::from(buf);
        let decoded = MeasuredValueNormalized::decode(&mut reader).unwrap();
        assert_eq!(mv, decoded);
    }

    #[test]
    fn normalized_no_quality_roundtrip() {
        let mv = MeasuredValueNormalizedNoQuality::new(32000);
        let mut buf = BytesMut::with_capacity(16);
        mv.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 2);

        let mut reader = Bytes::from(buf);
        let decoded = MeasuredValueNormalizedNoQuality::decode(&mut reader).unwrap();
        assert_eq!(mv, decoded);
    }

    #[test]
    fn buffer_too_short() {
        let mut buf = Bytes::from_static(&[0x00]);
        assert!(MeasuredValueNormalized::decode(&mut buf).is_err());

        let mut buf = Bytes::from_static(&[]);
        assert!(MeasuredValueNormalizedNoQuality::decode(&mut buf).is_err());
    }
}
