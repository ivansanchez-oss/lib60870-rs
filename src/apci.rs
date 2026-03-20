use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::FrameError;

const START_BYTE: u8 = 0x68;
const CONTROL_FIELD_SIZE: usize = 4;
const MAX_APDU_LENGTH: usize = 253;

/// U-frame function codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UFunction {
    StartDtAct,
    StartDtCon,
    StopDtAct,
    StopDtCon,
    TestFrAct,
    TestFrCon,
}

impl UFunction {
    fn to_byte(self) -> u8 {
        match self {
            Self::StartDtAct => 0x07,
            Self::StartDtCon => 0x0B,
            Self::StopDtAct => 0x13,
            Self::StopDtCon => 0x23,
            Self::TestFrAct => 0x43,
            Self::TestFrCon => 0x83,
        }
    }

    fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x07 => Some(Self::StartDtAct),
            0x0B => Some(Self::StartDtCon),
            0x13 => Some(Self::StopDtAct),
            0x23 => Some(Self::StopDtCon),
            0x43 => Some(Self::TestFrAct),
            0x83 => Some(Self::TestFrCon),
            _ => None,
        }
    }
}

/// APCI frame types used by IEC 60870-5-104.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Apdu {
    /// Information transfer frame carrying an ASDU payload.
    I {
        send_seq: u16,
        recv_seq: u16,
        payload: Bytes,
    },
    /// Supervisory frame acknowledging received I-frames.
    S { recv_seq: u16 },
    /// Unnumbered control frame.
    U(UFunction),
}

/// Incremental APCI frame parser with a pre-allocated buffer.
///
/// Accumulates bytes from the transport and yields complete [`Apdu`] frames.
/// Avoids per-frame allocation by reusing an internal `BytesMut`.
pub struct FrameReader {
    buf: BytesMut,
}

impl FrameReader {
    /// Create a new reader with a pre-allocated buffer.
    pub fn new() -> Self {
        // 2 (header) + MAX_APDU_LENGTH is the largest possible frame
        Self {
            buf: BytesMut::with_capacity(2 + MAX_APDU_LENGTH),
        }
    }

    /// Read one APDU from the stream.
    ///
    /// Reads incrementally into an internal buffer. Returns the next
    /// complete frame, or a `FrameError` on protocol/IO errors.
    pub async fn read_frame(
        &mut self,
        reader: &mut (impl AsyncRead + Unpin),
    ) -> Result<Apdu, FrameError> {
        loop {
            // Try to parse a complete frame from buffered data
            if let Some(apdu) = self.try_parse()? {
                return Ok(apdu);
            }

            // Need more data — read into the buffer
            let n = reader.read_buf(&mut self.buf).await?;
            if n == 0 {
                return Err(FrameError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed",
                )));
            }
        }
    }

    /// Try to parse one complete APDU from the buffer.
    ///
    /// Returns `Ok(None)` if there aren't enough bytes yet.
    fn try_parse(&mut self) -> Result<Option<Apdu>, FrameError> {
        if self.buf.remaining() < 2 {
            return Ok(None);
        }

        let start = self.buf[0];
        if start != START_BYTE {
            // Consume the bad byte and report the error
            self.buf.advance(1);
            return Err(FrameError::InvalidStartByte(start));
        }

        let length = self.buf[1] as usize;
        if length < CONTROL_FIELD_SIZE {
            // Consume both header bytes
            self.buf.advance(2);
            return Err(FrameError::LengthTooShort(length));
        }
        if length > MAX_APDU_LENGTH {
            self.buf.advance(2);
            return Err(FrameError::LengthExceeded {
                length,
                max: MAX_APDU_LENGTH,
            });
        }

        // Check if we have the full frame body
        let frame_size = 2 + length;
        if self.buf.remaining() < frame_size {
            return Ok(None);
        }

        // Consume header
        self.buf.advance(2);

        // Parse control field
        let cf1 = self.buf[0];
        let cf2 = self.buf[1];
        let cf3 = self.buf[2];
        let cf4 = self.buf[3];

        let apdu = if cf1 & 0x01 == 0 {
            // I-frame
            let send_seq = (u16::from(cf1) | (u16::from(cf2) << 8)) >> 1;
            let recv_seq = (u16::from(cf3) | (u16::from(cf4) << 8)) >> 1;
            self.buf.advance(CONTROL_FIELD_SIZE);
            let payload = self.buf.split_to(length - CONTROL_FIELD_SIZE).freeze();
            Apdu::I {
                send_seq,
                recv_seq,
                payload,
            }
        } else if cf1 & 0x03 == 0x01 {
            // S-frame
            let recv_seq = (u16::from(cf3) | (u16::from(cf4) << 8)) >> 1;
            self.buf.advance(length);
            Apdu::S { recv_seq }
        } else if cf1 & 0x03 == 0x03 {
            // U-frame
            let func = UFunction::from_byte(cf1).ok_or(FrameError::UnknownUFunction(cf1))?;
            self.buf.advance(length);
            Apdu::U(func)
        } else {
            self.buf.advance(length);
            return Err(FrameError::InvalidControlField(cf1));
        };

        Ok(Some(apdu))
    }
}

impl Default for FrameReader {
    fn default() -> Self {
        Self::new()
    }
}

/// Write one APDU to the stream.
pub async fn write_apdu(
    writer: &mut (impl AsyncWrite + Unpin),
    apdu: &Apdu,
) -> Result<(), FrameError> {
    match apdu {
        Apdu::I {
            send_seq,
            recv_seq,
            payload,
        } => {
            let length = CONTROL_FIELD_SIZE + payload.len();
            let mut buf = BytesMut::with_capacity(2 + length);
            buf.put_u8(START_BYTE);
            buf.put_u8(length as u8);
            let s = send_seq << 1;
            buf.put_u8(s as u8);
            buf.put_u8((s >> 8) as u8);
            let r = recv_seq << 1;
            buf.put_u8(r as u8);
            buf.put_u8((r >> 8) as u8);
            buf.extend_from_slice(payload);
            writer.write_all(&buf).await?;
        }
        Apdu::S { recv_seq } => {
            let r = recv_seq << 1;
            let buf = [START_BYTE, 0x04, 0x01, 0x00, r as u8, (r >> 8) as u8];
            writer.write_all(&buf).await?;
        }
        Apdu::U(func) => {
            let buf = [START_BYTE, 0x04, func.to_byte(), 0x00, 0x00, 0x00];
            writer.write_all(&buf).await?;
        }
    }
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    async fn write_and_read(apdu: &Apdu) -> Apdu {
        let (mut client, mut server) = duplex(256);
        write_apdu(&mut client, apdu).await.unwrap();
        let mut reader = FrameReader::new();
        reader.read_frame(&mut server).await.unwrap()
    }

    #[tokio::test]
    async fn roundtrip_u_frame() {
        for func in [
            UFunction::StartDtAct,
            UFunction::StartDtCon,
            UFunction::StopDtAct,
            UFunction::StopDtCon,
            UFunction::TestFrAct,
            UFunction::TestFrCon,
        ] {
            let decoded = write_and_read(&Apdu::U(func)).await;
            assert_eq!(decoded, Apdu::U(func));
        }
    }

    #[tokio::test]
    async fn roundtrip_s_frame() {
        let apdu = Apdu::S { recv_seq: 1234 };
        assert_eq!(write_and_read(&apdu).await, apdu);
    }

    #[tokio::test]
    async fn roundtrip_i_frame() {
        let payload = Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        let apdu = Apdu::I {
            send_seq: 100,
            recv_seq: 50,
            payload: payload.clone(),
        };
        assert_eq!(write_and_read(&apdu).await, apdu);
    }

    #[tokio::test]
    async fn roundtrip_max_sequence_numbers() {
        let apdu = Apdu::I {
            send_seq: 32767,
            recv_seq: 32767,
            payload: Bytes::from_static(&[0xFF]),
        };
        assert_eq!(write_and_read(&apdu).await, apdu);
    }

    #[tokio::test]
    async fn invalid_start_byte() {
        let (mut client, mut server) = duplex(64);
        client
            .write_all(&[0x99, 0x04, 0x07, 0x00, 0x00, 0x00])
            .await
            .unwrap();
        client.flush().await.unwrap();
        let mut reader = FrameReader::new();
        let err = reader.read_frame(&mut server).await.unwrap_err();
        assert!(matches!(err, FrameError::InvalidStartByte(0x99)));
    }

    #[tokio::test]
    async fn multiple_frames_in_buffer() {
        let (mut client, mut server) = duplex(256);
        // Write two U-frames back to back
        write_apdu(&mut client, &Apdu::U(UFunction::TestFrAct))
            .await
            .unwrap();
        write_apdu(&mut client, &Apdu::U(UFunction::TestFrCon))
            .await
            .unwrap();

        let mut reader = FrameReader::new();
        let f1 = reader.read_frame(&mut server).await.unwrap();
        let f2 = reader.read_frame(&mut server).await.unwrap();
        assert_eq!(f1, Apdu::U(UFunction::TestFrAct));
        assert_eq!(f2, Apdu::U(UFunction::TestFrCon));
    }

    #[tokio::test]
    async fn try_parse_returns_none_on_empty() {
        let mut reader = FrameReader::new();
        assert!(reader.try_parse().unwrap().is_none());
    }

    #[tokio::test]
    async fn try_parse_returns_none_on_partial() {
        let mut reader = FrameReader::new();
        // Only the start byte and length, but not the full body
        reader.buf.extend_from_slice(&[START_BYTE, 0x04]);
        assert!(reader.try_parse().unwrap().is_none());
        // Data still in buffer for next read
        assert_eq!(reader.buf.len(), 2);
    }
}
