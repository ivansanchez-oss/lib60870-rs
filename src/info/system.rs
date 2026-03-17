use bytes::{Buf, BufMut};

use crate::error::{Error, Result};
use crate::types::Cp56Time2a;

use super::traits::{Decode, Encode};

/// Interrogation command — qualifier of interrogation (QOI).
///
/// Wire format: 1 byte (QOI value, typically 20 = station interrogation)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterrogationCommand {
    pub qoi: u8,
}

impl InterrogationCommand {
    pub const ENCODED_SIZE: usize = 1;

    pub fn new(qoi: u8) -> Self {
        Self { qoi }
    }

    /// Station interrogation (QOI = 20).
    pub fn station() -> Self {
        Self { qoi: 20 }
    }
}

impl Encode for InterrogationCommand {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        buf.put_u8(self.qoi);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for InterrogationCommand {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        Ok(Self { qoi: buf.get_u8() })
    }
}

/// Counter interrogation command — qualifier of counter interrogation (QCC).
///
/// Wire format: 1 byte
///   - bits 0-4: request (RQT)
///   - bits 5-6: freeze (FRZ)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterInterrogationCommand {
    pub qcc: u8,
}

impl CounterInterrogationCommand {
    pub const ENCODED_SIZE: usize = 1;

    pub fn new(qcc: u8) -> Self {
        Self { qcc }
    }
}

impl Encode for CounterInterrogationCommand {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        buf.put_u8(self.qcc);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for CounterInterrogationCommand {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        Ok(Self { qcc: buf.get_u8() })
    }
}

/// Read command — no payload.
///
/// Wire format: 0 bytes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadCommand;

impl ReadCommand {
    pub const ENCODED_SIZE: usize = 0;
}

impl Encode for ReadCommand {
    fn encode(&self, _buf: &mut impl BufMut) -> Result<()> {
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for ReadCommand {
    fn decode(_buf: &mut impl Buf) -> Result<Self> {
        Ok(Self)
    }
}

/// Clock synchronization command — contains CP56Time2a.
///
/// Wire format: 7 bytes (Cp56Time2a)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockSyncCommand {
    pub time: Cp56Time2a,
}

impl ClockSyncCommand {
    pub const ENCODED_SIZE: usize = Cp56Time2a::ENCODED_SIZE;

    pub fn new(time: Cp56Time2a) -> Self {
        Self { time }
    }
}

impl Encode for ClockSyncCommand {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        buf.put_slice(self.time.as_bytes());
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for ClockSyncCommand {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let mut bytes = [0u8; 7];
        buf.copy_to_slice(&mut bytes);
        let time = Cp56Time2a::from_bytes(&bytes)?;
        Ok(Self { time })
    }
}

/// End of initialization — cause of initialization (COI).
///
/// Wire format: 1 byte
///   - bits 0-6: cause (0=local power on, 1=local manual reset, 2=remote reset)
///   - bit 7: change of local parameters (1=yes)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EndOfInitialization {
    pub cause: u8,
    pub local_param_change: bool,
}

impl EndOfInitialization {
    pub const ENCODED_SIZE: usize = 1;

    pub fn new(cause: u8, local_param_change: bool) -> Self {
        Self {
            cause: cause & 0x7F,
            local_param_change,
        }
    }
}

impl Encode for EndOfInitialization {
    fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        if buf.remaining_mut() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining_mut(),
            });
        }
        let byte = (self.cause & 0x7F) | ((self.local_param_change as u8) << 7);
        buf.put_u8(byte);
        Ok(())
    }

    fn encoded_size(&self) -> usize {
        Self::ENCODED_SIZE
    }
}

impl Decode for EndOfInitialization {
    fn decode(buf: &mut impl Buf) -> Result<Self> {
        if buf.remaining() < Self::ENCODED_SIZE {
            return Err(Error::BufferTooShort {
                need: Self::ENCODED_SIZE,
                have: buf.remaining(),
            });
        }
        let byte = buf.get_u8();
        Ok(Self {
            cause: byte & 0x7F,
            local_param_change: byte & 0x80 != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn interrogation_roundtrip() {
        let cmd = InterrogationCommand::station();
        let mut buf = BytesMut::with_capacity(16);
        cmd.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = InterrogationCommand::decode(&mut reader).unwrap();
        assert_eq!(decoded.qoi, 20);
    }

    #[test]
    fn counter_interrogation_roundtrip() {
        let cmd = CounterInterrogationCommand::new(0x45);
        let mut buf = BytesMut::with_capacity(16);
        cmd.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = CounterInterrogationCommand::decode(&mut reader).unwrap();
        assert_eq!(cmd, decoded);
    }

    #[test]
    fn read_command_roundtrip() {
        let cmd = ReadCommand;
        let mut buf = BytesMut::with_capacity(16);
        cmd.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 0);
        let mut reader = Bytes::from(buf);
        let decoded = ReadCommand::decode(&mut reader).unwrap();
        assert_eq!(cmd, decoded);
    }

    #[test]
    fn clock_sync_roundtrip() {
        let mut time = Cp56Time2a::new();
        time.set_year(25);
        time.set_month(3);
        time.set_day_of_month(17);
        time.set_hour(14);
        time.set_minute(30);
        time.set_second(45);
        time.set_millisecond(123);

        let cmd = ClockSyncCommand::new(time);
        let mut buf = BytesMut::with_capacity(16);
        cmd.encode(&mut buf).unwrap();
        assert_eq!(buf.len(), 7);

        let mut reader = Bytes::from(buf);
        let decoded = ClockSyncCommand::decode(&mut reader).unwrap();
        assert_eq!(cmd, decoded);
    }

    #[test]
    fn end_of_init_roundtrip() {
        let eoi = EndOfInitialization::new(2, true);
        let mut buf = BytesMut::with_capacity(16);
        eoi.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = EndOfInitialization::decode(&mut reader).unwrap();
        assert_eq!(eoi, decoded);
    }

    #[test]
    fn end_of_init_flags() {
        let eoi = EndOfInitialization::new(1, false);
        let mut buf = BytesMut::with_capacity(16);
        eoi.encode(&mut buf).unwrap();
        assert_eq!(buf[0], 0x01);
    }
}
