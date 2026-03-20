use bytes::{Buf, BufMut};

use crate::error::AduError;

use super::traits::{Decode, Encode};

/// Single command (SCO).
///
/// Wire format: 1 byte
///   - bit 0: SCS (state, false=OFF, true=ON)
///   - bits 2-6: QU (qualifier of command, 0-31)
///   - bit 7: S/E (select=1 / execute=0)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleCommand {
    pub state: bool,
    pub qualifier: u8,
    pub select: bool,
}

impl SingleCommand {
    pub const ENCODED_SIZE: usize = 1;

    pub fn new(state: bool, qualifier: u8, select: bool) -> Self {
        Self {
            state,
            qualifier: qualifier & 0x1F,
            select,
        }
    }
}

impl Encode for SingleCommand {
    fn encode(&self, buf: &mut impl BufMut) -> Result<(), AduError> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(AduError::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        let byte = (self.state as u8)
            | ((self.qualifier & 0x1F) << 2)
            | ((self.select as u8) << 7);
        buf.put_u8(byte);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for SingleCommand {
    fn decode(buf: &mut impl Buf) -> Result<Self, AduError> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(AduError::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let byte = buf.get_u8();
        Ok(Self {
            state: byte & 0x01 != 0,
            qualifier: (byte >> 2) & 0x1F,
            select: byte & 0x80 != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn roundtrip() {
        let cmd = SingleCommand::new(true, 5, true);
        let mut buf = BytesMut::with_capacity(16);
        cmd.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 1);

        let mut reader = Bytes::from(buf);
        let decoded = SingleCommand::decode(&mut reader).unwrap();
        assert_eq!(cmd, decoded);
    }

    #[test]
    fn off_execute() {
        let cmd = SingleCommand::new(false, 0, false);
        let mut buf = BytesMut::with_capacity(16);
        cmd.encode(&mut buf).unwrap();
        assert_eq!(buf[0], 0x00);

        let mut reader = Bytes::from(buf);
        let decoded = SingleCommand::decode(&mut reader).unwrap();
        assert!(!decoded.state);
        assert_eq!(decoded.qualifier, 0);
        assert!(!decoded.select);
    }

    #[test]
    fn buffer_too_short() {
        let mut buf = Bytes::new();
        assert!(SingleCommand::decode(&mut buf).is_err());
    }
}
