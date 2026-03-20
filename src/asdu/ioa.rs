use bytes::{Buf, BufMut};

use crate::error::{AduError, IoaOverflow};

/// Maximum value for a 3-byte IOA.
const IOA_MAX: u32 = 0xFFFFFF;

/// Address of an information object within an ASDU (1-3 bytes on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InformationObjectAddress(u32);

impl InformationObjectAddress {
    /// Create a new IOA, validating that it fits in 3 bytes (max 0xFFFFFF).
    pub fn try_new(value: u32) -> Result<Self, IoaOverflow> {
        if value > IOA_MAX {
            return Err(IoaOverflow(value));
        }
        Ok(Self(value))
    }

    pub fn value(self) -> u32 {
        self.0
    }

    /// Encode the IOA in little-endian using `size_of_ioa` bytes (1-3).
    pub fn encode(&self, buf: &mut impl BufMut, size_of_ioa: u8) -> Result<(), AduError> {
        let size = size_of_ioa as usize;
        if buf.remaining_mut() < size {
            return Err(AduError::BufferTooShort {
                need: size,
                have: buf.remaining_mut(),
            });
        }
        let bytes = self.0.to_le_bytes();
        buf.put_slice(&bytes[..size]);
        Ok(())
    }

    /// Decode the IOA from little-endian `size_of_ioa` bytes (1-3).
    pub fn decode(buf: &mut impl Buf, size_of_ioa: u8) -> Result<Self, AduError> {
        let size = size_of_ioa as usize;
        if buf.remaining() < size {
            return Err(AduError::BufferTooShort {
                need: size,
                have: buf.remaining(),
            });
        }
        let mut bytes = [0u8; 4];
        buf.copy_to_slice(&mut bytes[..size]);
        Ok(Self(u32::from_le_bytes(bytes)))
    }
}

impl TryFrom<u32> for InformationObjectAddress {
    type Error = IoaOverflow;

    fn try_from(value: u32) -> Result<Self, IoaOverflow> {
        Self::try_new(value)
    }
}

impl From<u16> for InformationObjectAddress {
    fn from(value: u16) -> Self {
        Self(value as u32)
    }
}

impl From<u8> for InformationObjectAddress {
    fn from(value: u8) -> Self {
        Self(value as u32)
    }
}

impl std::fmt::Display for InformationObjectAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn try_new_valid() {
        let ioa = InformationObjectAddress::try_new(0xFFFFFF).unwrap();
        assert_eq!(ioa.value(), 0xFFFFFF);
    }

    #[test]
    fn try_new_zero() {
        let ioa = InformationObjectAddress::try_new(0).unwrap();
        assert_eq!(ioa.value(), 0);
    }

    #[test]
    fn try_new_rejects_overflow() {
        assert!(InformationObjectAddress::try_new(0x1000000).is_err());
        assert!(InformationObjectAddress::try_new(u32::MAX).is_err());
    }

    #[test]
    fn try_from_u32() {
        let ioa: InformationObjectAddress = 42u32.try_into().unwrap();
        assert_eq!(ioa.value(), 42);
    }

    #[test]
    fn try_from_u32_overflow() {
        let result: std::result::Result<InformationObjectAddress, _> = 0x1000000u32.try_into();
        assert!(result.is_err());
    }

    #[test]
    fn from_u16() {
        let ioa: InformationObjectAddress = 1000u16.into();
        assert_eq!(ioa.value(), 1000);
    }

    #[test]
    fn from_u8() {
        let ioa: InformationObjectAddress = 255u8.into();
        assert_eq!(ioa.value(), 255);
    }

    #[test]
    fn roundtrip_1_byte() {
        let ioa = InformationObjectAddress::try_new(0xAB).unwrap();
        let mut buf = BytesMut::with_capacity(4);
        ioa.encode(&mut buf, 1).unwrap();
        assert_eq!(buf.len(), 1);
        let mut reader = Bytes::from(buf);
        let decoded = InformationObjectAddress::decode(&mut reader, 1).unwrap();
        assert_eq!(decoded.value(), 0xAB);
    }

    #[test]
    fn roundtrip_2_bytes() {
        let ioa = InformationObjectAddress::try_new(0x1234).unwrap();
        let mut buf = BytesMut::with_capacity(4);
        ioa.encode(&mut buf, 2).unwrap();
        assert_eq!(buf.len(), 2);
        let mut reader = Bytes::from(buf);
        let decoded = InformationObjectAddress::decode(&mut reader, 2).unwrap();
        assert_eq!(decoded.value(), 0x1234);
    }

    #[test]
    fn roundtrip_3_bytes() {
        let ioa = InformationObjectAddress::try_new(0x123456).unwrap();
        let mut buf = BytesMut::with_capacity(4);
        ioa.encode(&mut buf, 3).unwrap();
        assert_eq!(buf.len(), 3);
        let mut reader = Bytes::from(buf);
        let decoded = InformationObjectAddress::decode(&mut reader, 3).unwrap();
        assert_eq!(decoded.value(), 0x123456);
    }

    #[test]
    fn buffer_too_short_encode() {
        let ioa = InformationObjectAddress::try_new(0).unwrap();
        let mut buf = [0u8; 1];
        let mut writer = &mut buf[..];
        assert!(ioa.encode(&mut writer, 3).is_err());
    }

    #[test]
    fn buffer_too_short_decode() {
        let mut buf = Bytes::from_static(&[0x01]);
        assert!(InformationObjectAddress::decode(&mut buf, 3).is_err());
    }
}
