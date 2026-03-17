use bytes::{Buf, BufMut};

use crate::error::{Error, Result};
use crate::types::QualityDescriptor;

use super::traits::{Decode, Encode};

/// Measured value, scaled (SVA + QDS).
///
/// Wire format: 3 bytes
///   - bytes 0-1: scaled value (i16, little-endian)
///   - byte 2: quality descriptor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasuredValueScaled {
    pub value: i16,
    pub quality: QualityDescriptor,
}

impl MeasuredValueScaled {
    pub const ENCODED_SIZE: usize = 3;

    pub fn new(value: i16, quality: QualityDescriptor) -> Self {
        Self { value, quality }
    }
}

impl Encode for MeasuredValueScaled {
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

impl Decode for MeasuredValueScaled {
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

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn roundtrip() {
        let mv = MeasuredValueScaled::new(-500, QualityDescriptor::NON_TOPICAL);
        let mut buf = BytesMut::with_capacity(16);
        mv.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 3);

        let mut reader = Bytes::from(buf);
        let decoded = MeasuredValueScaled::decode(&mut reader).unwrap();
        assert_eq!(mv, decoded);
    }

    #[test]
    fn buffer_too_short() {
        let mut buf = Bytes::from_static(&[0x00]);
        assert!(MeasuredValueScaled::decode(&mut buf).is_err());
    }
}
