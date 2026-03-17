use std::io;

use bytes::{BufMut, Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

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

/// Read one APDU from the stream.
pub async fn read_apdu(reader: &mut (impl AsyncRead + Unpin)) -> io::Result<Apdu> {
    let start = reader.read_u8().await?;
    if start != START_BYTE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("expected start byte 0x68, got 0x{:02X}", start),
        ));
    }

    let length = reader.read_u8().await? as usize;
    if length < CONTROL_FIELD_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "APDU length too short",
        ));
    }
    if length > MAX_APDU_LENGTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("APDU length {} exceeds maximum {}", length, MAX_APDU_LENGTH),
        ));
    }

    let mut buf = vec![0u8; length];
    reader.read_exact(&mut buf).await?;

    let cf1 = buf[0];
    let cf2 = buf[1];
    let cf3 = buf[2];
    let cf4 = buf[3];

    if cf1 & 0x01 == 0 {
        // I-frame
        let send_seq = (u16::from(cf1) | (u16::from(cf2) << 8)) >> 1;
        let recv_seq = (u16::from(cf3) | (u16::from(cf4) << 8)) >> 1;
        let payload = Bytes::copy_from_slice(&buf[4..]);
        Ok(Apdu::I {
            send_seq,
            recv_seq,
            payload,
        })
    } else if cf1 & 0x03 == 0x01 {
        // S-frame
        let recv_seq = (u16::from(cf3) | (u16::from(cf4) << 8)) >> 1;
        Ok(Apdu::S { recv_seq })
    } else if cf1 & 0x03 == 0x03 {
        // U-frame
        UFunction::from_byte(cf1).map(Apdu::U).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unknown U-frame control byte 0x{:02X}", cf1),
            )
        })
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid control field byte 0x{:02X}", cf1),
        ))
    }
}

/// Write one APDU to the stream.
pub async fn write_apdu(writer: &mut (impl AsyncWrite + Unpin), apdu: &Apdu) -> io::Result<()> {
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
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

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
            let (mut client, mut server) = duplex(64);
            write_apdu(&mut client, &Apdu::U(func)).await.unwrap();
            let frame = read_apdu(&mut server).await.unwrap();
            assert_eq!(frame, Apdu::U(func));
        }
    }

    #[tokio::test]
    async fn roundtrip_s_frame() {
        let (mut client, mut server) = duplex(64);
        let apdu = Apdu::S { recv_seq: 1234 };
        write_apdu(&mut client, &apdu).await.unwrap();
        let decoded = read_apdu(&mut server).await.unwrap();
        assert_eq!(decoded, apdu);
    }

    #[tokio::test]
    async fn roundtrip_i_frame() {
        let (mut client, mut server) = duplex(256);
        let payload = Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        let apdu = Apdu::I {
            send_seq: 100,
            recv_seq: 50,
            payload: payload.clone(),
        };
        write_apdu(&mut client, &apdu).await.unwrap();
        let decoded = read_apdu(&mut server).await.unwrap();
        assert_eq!(decoded, apdu);
    }

    #[tokio::test]
    async fn roundtrip_max_sequence_numbers() {
        let (mut client, mut server) = duplex(256);
        // Max sequence number is 32767 (15 bits)
        let apdu = Apdu::I {
            send_seq: 32767,
            recv_seq: 32767,
            payload: Bytes::from_static(&[0xFF]),
        };
        write_apdu(&mut client, &apdu).await.unwrap();
        let decoded = read_apdu(&mut server).await.unwrap();
        assert_eq!(decoded, apdu);
    }

    #[tokio::test]
    async fn invalid_start_byte() {
        let (mut client, mut server) = duplex(64);
        client
            .write_all(&[0x99, 0x04, 0x07, 0x00, 0x00, 0x00])
            .await
            .unwrap();
        client.flush().await.unwrap();
        let err = read_apdu(&mut server).await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
