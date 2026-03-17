use bytes::{Buf, BufMut};

use crate::error::{Error, Result};

/// Address of an information object within an ASDU (1-3 bytes on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InformationObjectAddress(u32);

impl InformationObjectAddress {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }

    /// Encode the IOA in little-endian using `size_of_ioa` bytes (1-3).
    pub fn encode(&self, buf: &mut impl BufMut, size_of_ioa: u8) -> Result<()> {
        let size = size_of_ioa as usize;
        if buf.remaining_mut() < size {
            return Err(Error::BufferTooShort {
                need: size,
                have: buf.remaining_mut(),
            });
        }
        let bytes = self.0.to_le_bytes();
        buf.put_slice(&bytes[..size]);
        Ok(())
    }

    /// Decode the IOA from little-endian `size_of_ioa` bytes (1-3).
    pub fn decode(buf: &mut impl Buf, size_of_ioa: u8) -> Result<Self> {
        let size = size_of_ioa as usize;
        if buf.remaining() < size {
            return Err(Error::BufferTooShort {
                need: size,
                have: buf.remaining(),
            });
        }
        let mut bytes = [0u8; 4];
        buf.copy_to_slice(&mut bytes[..size]);
        Ok(Self(u32::from_le_bytes(bytes)))
    }
}

impl From<u32> for InformationObjectAddress {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<u16> for InformationObjectAddress {
    fn from(value: u16) -> Self {
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
    fn roundtrip_1_byte() {
        let ioa = InformationObjectAddress::new(0xAB);
        let mut buf = BytesMut::with_capacity(4);
        ioa.encode(&mut buf, 1).unwrap();
        assert_eq!(buf.len(), 1);
        let mut reader = Bytes::from(buf);
        let decoded = InformationObjectAddress::decode(&mut reader, 1).unwrap();
        assert_eq!(decoded.value(), 0xAB);
    }

    #[test]
    fn roundtrip_2_bytes() {
        let ioa = InformationObjectAddress::new(0x1234);
        let mut buf = BytesMut::with_capacity(4);
        ioa.encode(&mut buf, 2).unwrap();
        assert_eq!(buf.len(), 2);
        let mut reader = Bytes::from(buf);
        let decoded = InformationObjectAddress::decode(&mut reader, 2).unwrap();
        assert_eq!(decoded.value(), 0x1234);
    }

    #[test]
    fn roundtrip_3_bytes() {
        let ioa = InformationObjectAddress::new(0x123456);
        let mut buf = BytesMut::with_capacity(4);
        ioa.encode(&mut buf, 3).unwrap();
        assert_eq!(buf.len(), 3);
        let mut reader = Bytes::from(buf);
        let decoded = InformationObjectAddress::decode(&mut reader, 3).unwrap();
        assert_eq!(decoded.value(), 0x123456);
    }

    #[test]
    fn from_u32() {
        let ioa: InformationObjectAddress = 42u32.into();
        assert_eq!(ioa.value(), 42);
    }

    #[test]
    fn from_u16() {
        let ioa: InformationObjectAddress = 1000u16.into();
        assert_eq!(ioa.value(), 1000);
    }

    #[test]
    fn buffer_too_short_encode() {
        let ioa = InformationObjectAddress::new(0);
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
