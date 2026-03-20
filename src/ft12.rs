use bytes::{Buf, BufMut, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::LinkError;

const START_FIXED: u8 = 0x10;
const START_VARIABLE: u8 = 0x68;
const END_BYTE: u8 = 0x16;
const SINGLE_ACK: u8 = 0xE5;

/// Maximum user data length in a variable-length FT 1.2 frame.
///
/// The L field is a single byte encoding the count of CF + ADDR + DATA bytes.
/// Per IEC 60870-5-2, the maximum value of L is 253.
const MAX_USER_DATA_LENGTH: usize = 253;

/// Link layer address (1 or 2 bytes depending on configuration).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LinkAddress(pub u16);

impl From<u8> for LinkAddress {
    fn from(v: u8) -> Self {
        Self(v as u16)
    }
}

impl From<u16> for LinkAddress {
    fn from(v: u16) -> Self {
        Self(v)
    }
}

/// Primary station function codes (PRM=1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PrimaryFunction {
    /// FC=0: Reset of remote link.
    ResetLink = 0,
    /// FC=3: Send/Confirm — user data with confirmation.
    SendConfirm = 3,
    /// FC=9: Request status of link.
    RequestLinkStatus = 9,
    /// FC=10: Request user data class 1 (high priority).
    RequestClass1 = 10,
    /// FC=11: Request user data class 2 (low priority).
    RequestClass2 = 11,
}

impl PrimaryFunction {
    fn from_u8(v: u8) -> Result<Self, LinkError> {
        match v {
            0 => Ok(Self::ResetLink),
            3 => Ok(Self::SendConfirm),
            9 => Ok(Self::RequestLinkStatus),
            10 => Ok(Self::RequestClass1),
            11 => Ok(Self::RequestClass2),
            _ => Err(LinkError::UnknownPrimaryFunction(v)),
        }
    }
}

/// Secondary station function codes (PRM=0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SecondaryFunction {
    /// FC=0: Acknowledgment (positive confirm).
    Ack = 0,
    /// FC=1: Nack (negative acknowledgment).
    Nack = 1,
    /// FC=8: User data — response with ASDU.
    UserData = 8,
    /// FC=9: No data — response without ASDU.
    NoData = 9,
    /// FC=11: Status of link / access demand.
    LinkStatus = 11,
}

impl SecondaryFunction {
    fn from_u8(v: u8) -> Result<Self, LinkError> {
        match v {
            0 => Ok(Self::Ack),
            1 => Ok(Self::Nack),
            8 => Ok(Self::UserData),
            9 => Ok(Self::NoData),
            11 => Ok(Self::LinkStatus),
            _ => Err(LinkError::UnknownSecondaryFunction(v)),
        }
    }
}

/// FT 1.2 control field.
///
/// The control byte encodes direction (PRM), toggle bits (FCB/FCV or ACD/DFC),
/// and function code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlField {
    /// Primary message (PRM=1): master → slave.
    Primary {
        /// Frame count bit — toggled after each successful confirmed exchange.
        fcb: bool,
        /// Frame count valid — indicates FCB should be evaluated.
        fcv: bool,
        /// Function code.
        function: PrimaryFunction,
    },
    /// Secondary message (PRM=0): slave → master.
    Secondary {
        /// Access demand — slave has class 1 data available.
        acd: bool,
        /// Data flow control — slave cannot accept further data.
        dfc: bool,
        /// Function code.
        function: SecondaryFunction,
    },
}

impl ControlField {
    /// Encode the control field into a single byte.
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Primary { fcb, fcv, function } => {
                let mut b = 0x40; // PRM=1 (bit 6)
                if fcb {
                    b |= 0x20; // FCB (bit 5)
                }
                if fcv {
                    b |= 0x10; // FCV (bit 4)
                }
                b | (function as u8)
            }
            Self::Secondary { acd, dfc, function } => {
                let mut b = 0x00; // PRM=0
                if acd {
                    b |= 0x20; // ACD (bit 5)
                }
                if dfc {
                    b |= 0x10; // DFC (bit 4)
                }
                b | (function as u8)
            }
        }
    }

    /// Decode a control field from a single byte.
    pub fn from_byte(b: u8) -> Result<Self, LinkError> {
        let prm = b & 0x40 != 0;
        let fc = b & 0x0F;

        if prm {
            Ok(Self::Primary {
                fcb: b & 0x20 != 0,
                fcv: b & 0x10 != 0,
                function: PrimaryFunction::from_u8(fc)?,
            })
        } else {
            Ok(Self::Secondary {
                acd: b & 0x20 != 0,
                dfc: b & 0x10 != 0,
                function: SecondaryFunction::from_u8(fc)?,
            })
        }
    }
}

/// FT 1.2 link-layer frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkFrame {
    /// Fixed-length frame: `0x10, CF, ADDR, CS, 0x16`.
    Fixed {
        control: ControlField,
        address: LinkAddress,
    },
    /// Variable-length frame: `0x68, L, L, 0x68, CF, ADDR, DATA..., CS, 0x16`.
    Variable {
        control: ControlField,
        address: LinkAddress,
        /// User data (ASDU payload).
        data: bytes::Bytes,
    },
    /// Single-character acknowledgment: `0xE5`.
    SingleAck,
}

/// Compute FT 1.2 checksum: sum of bytes mod 256.
fn checksum(data: &[u8]) -> u8 {
    data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b))
}

/// Encode a link address into bytes.
fn encode_address(addr: LinkAddress, link_addr_size: usize, buf: &mut BytesMut) {
    buf.put_u8(addr.0 as u8);
    if link_addr_size == 2 {
        buf.put_u8((addr.0 >> 8) as u8);
    }
}

/// Incremental FT 1.2 frame parser with a pre-allocated buffer.
///
/// Accumulates bytes from the transport and yields complete [`LinkFrame`]s.
/// Avoids per-frame allocation by reusing an internal `BytesMut`.
pub struct LinkFrameParser {
    buf: BytesMut,
    addr_size: usize,
}

impl LinkFrameParser {
    /// Create a new parser for the given link address size (1 or 2 bytes).
    pub fn new(link_addr_size: u8) -> Self {
        Self {
            buf: BytesMut::with_capacity(4 + MAX_USER_DATA_LENGTH + 2),
            addr_size: link_addr_size as usize,
        }
    }

    /// Read one FT 1.2 frame from the stream.
    pub async fn read_frame(
        &mut self,
        reader: &mut (impl AsyncRead + Unpin),
    ) -> Result<LinkFrame, LinkError> {
        loop {
            if let Some(frame) = self.try_parse()? {
                return Ok(frame);
            }
            let n = reader.read_buf(&mut self.buf).await?;
            if n == 0 {
                return Err(LinkError::Io(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "connection closed",
                )));
            }
        }
    }

    fn try_parse(&mut self) -> Result<Option<LinkFrame>, LinkError> {
        if self.buf.is_empty() {
            return Ok(None);
        }

        match self.buf[0] {
            SINGLE_ACK => {
                self.buf.advance(1);
                Ok(Some(LinkFrame::SingleAck))
            }
            START_FIXED => self.try_parse_fixed(),
            START_VARIABLE => self.try_parse_variable(),
            other => {
                self.buf.advance(1);
                Err(LinkError::InvalidStartByte(other))
            }
        }
    }

    /// Parse fixed-length frame: `0x10, CF, ADDR(1-2), CS, 0x16`.
    fn try_parse_fixed(&mut self) -> Result<Option<LinkFrame>, LinkError> {
        let frame_size = 1 + 1 + self.addr_size + 1 + 1;
        if self.buf.remaining() < frame_size {
            return Ok(None);
        }

        let end = self.buf[frame_size - 1];
        if end != END_BYTE {
            self.buf.advance(1);
            return Err(LinkError::InvalidEndByte(end));
        }

        // Checksum covers CF + ADDR bytes
        let body = &self.buf[1..1 + 1 + self.addr_size];
        let expected_cs = checksum(body);
        let cs = self.buf[1 + 1 + self.addr_size];

        if cs != expected_cs {
            self.buf.advance(frame_size);
            return Err(LinkError::ChecksumMismatch {
                expected: expected_cs,
                got: cs,
            });
        }

        let control = ControlField::from_byte(self.buf[1])?;
        let address = if self.addr_size == 1 {
            LinkAddress(self.buf[2] as u16)
        } else {
            LinkAddress(u16::from(self.buf[2]) | (u16::from(self.buf[3]) << 8))
        };

        self.buf.advance(frame_size);
        Ok(Some(LinkFrame::Fixed { control, address }))
    }

    /// Parse variable-length frame: `0x68, L, L, 0x68, CF, ADDR(1-2), DATA..., CS, 0x16`.
    fn try_parse_variable(&mut self) -> Result<Option<LinkFrame>, LinkError> {
        if self.buf.remaining() < 4 {
            return Ok(None);
        }

        let l1 = self.buf[1] as usize;
        let l2 = self.buf[2] as usize;

        if l1 != l2 {
            self.buf.advance(1);
            return Err(LinkError::LengthMismatch {
                l1: l1 as u8,
                l2: l2 as u8,
            });
        }

        let min_length = 1 + self.addr_size; // CF + ADDR
        if l1 < min_length {
            self.buf.advance(4);
            return Err(LinkError::LengthTooShort(l1));
        }

        if l1 > MAX_USER_DATA_LENGTH {
            self.buf.advance(4);
            return Err(LinkError::LengthExceeded(l1));
        }

        // Total: 4(header) + L(body) + 1(CS) + 1(end)
        let total = 4 + l1 + 2;
        if self.buf.remaining() < total {
            return Ok(None);
        }

        if self.buf[3] != START_VARIABLE {
            self.buf.advance(1);
            return Err(LinkError::InvalidStartByte(self.buf[2]));
        }

        let end = self.buf[total - 1];
        if end != END_BYTE {
            self.buf.advance(1);
            return Err(LinkError::InvalidEndByte(end));
        }

        // Body: CF + ADDR + DATA at offset 4..4+l1
        let body = &self.buf[4..4 + l1];
        let expected_cs = checksum(body);
        let cs = self.buf[4 + l1];

        if cs != expected_cs {
            self.buf.advance(total);
            return Err(LinkError::ChecksumMismatch {
                expected: expected_cs,
                got: cs,
            });
        }

        let control = ControlField::from_byte(body[0])?;
        let address = if self.addr_size == 1 {
            LinkAddress(body[1] as u16)
        } else {
            LinkAddress(u16::from(body[1]) | (u16::from(body[2]) << 8))
        };

        let data_start = 1 + self.addr_size;
        let data = BytesMut::from(&body[data_start..]).freeze();

        self.buf.advance(total);
        Ok(Some(LinkFrame::Variable {
            control,
            address,
            data,
        }))
    }
}

impl Default for LinkFrameParser {
    fn default() -> Self {
        Self::new(1)
    }
}

/// Write one FT 1.2 frame to the stream.
pub async fn write_link_frame(
    writer: &mut (impl AsyncWrite + Unpin),
    frame: &LinkFrame,
    link_addr_size: u8,
) -> Result<(), LinkError> {
    let addr_size = link_addr_size as usize;
    match frame {
        LinkFrame::SingleAck => {
            writer.write_all(&[SINGLE_ACK]).await?;
        }
        LinkFrame::Fixed { control, address } => {
            let cf = control.to_byte();
            let mut body = BytesMut::with_capacity(1 + addr_size);
            body.put_u8(cf);
            encode_address(*address, addr_size, &mut body);
            let cs = checksum(&body);

            let mut buf = BytesMut::with_capacity(1 + body.len() + 2);
            buf.put_u8(START_FIXED);
            buf.extend_from_slice(&body);
            buf.put_u8(cs);
            buf.put_u8(END_BYTE);
            writer.write_all(&buf).await?;
        }
        LinkFrame::Variable {
            control,
            address,
            data,
        } => {
            let cf = control.to_byte();
            let l = 1 + addr_size + data.len();

            let mut body = BytesMut::with_capacity(l);
            body.put_u8(cf);
            encode_address(*address, addr_size, &mut body);
            body.extend_from_slice(data);
            let cs = checksum(&body);

            let mut buf = BytesMut::with_capacity(4 + l + 2);
            buf.put_u8(START_VARIABLE);
            buf.put_u8(l as u8);
            buf.put_u8(l as u8);
            buf.put_u8(START_VARIABLE);
            buf.extend_from_slice(&body);
            buf.put_u8(cs);
            buf.put_u8(END_BYTE);
            writer.write_all(&buf).await?;
        }
    }
    writer.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use tokio::io::duplex;

    async fn write_and_read(frame: &LinkFrame, addr_size: u8) -> LinkFrame {
        let (mut client, mut server) = duplex(256);
        write_link_frame(&mut client, frame, addr_size)
            .await
            .unwrap();
        let mut parser = LinkFrameParser::new(addr_size);
        parser.read_frame(&mut server).await.unwrap()
    }

    #[tokio::test]
    async fn roundtrip_single_ack() {
        let decoded = write_and_read(&LinkFrame::SingleAck, 1).await;
        assert_eq!(decoded, LinkFrame::SingleAck);
    }

    #[tokio::test]
    async fn roundtrip_fixed_primary_1byte_addr() {
        let frame = LinkFrame::Fixed {
            control: ControlField::Primary {
                fcb: true,
                fcv: false,
                function: PrimaryFunction::ResetLink,
            },
            address: LinkAddress(5),
        };
        assert_eq!(write_and_read(&frame, 1).await, frame);
    }

    #[tokio::test]
    async fn roundtrip_fixed_primary_2byte_addr() {
        let frame = LinkFrame::Fixed {
            control: ControlField::Primary {
                fcb: false,
                fcv: true,
                function: PrimaryFunction::RequestLinkStatus,
            },
            address: LinkAddress(0x0102),
        };
        assert_eq!(write_and_read(&frame, 2).await, frame);
    }

    #[tokio::test]
    async fn roundtrip_fixed_secondary() {
        let frame = LinkFrame::Fixed {
            control: ControlField::Secondary {
                acd: true,
                dfc: false,
                function: SecondaryFunction::Ack,
            },
            address: LinkAddress(1),
        };
        assert_eq!(write_and_read(&frame, 1).await, frame);
    }

    #[tokio::test]
    async fn roundtrip_variable_1byte_addr() {
        let frame = LinkFrame::Variable {
            control: ControlField::Primary {
                fcb: true,
                fcv: true,
                function: PrimaryFunction::SendConfirm,
            },
            address: LinkAddress(10),
            data: Bytes::from_static(&[0x01, 0x02, 0x03, 0x04]),
        };
        assert_eq!(write_and_read(&frame, 1).await, frame);
    }

    #[tokio::test]
    async fn roundtrip_variable_2byte_addr() {
        let frame = LinkFrame::Variable {
            control: ControlField::Secondary {
                acd: false,
                dfc: false,
                function: SecondaryFunction::UserData,
            },
            address: LinkAddress(0x1234),
            data: Bytes::from_static(&[0xAA, 0xBB, 0xCC]),
        };
        assert_eq!(write_and_read(&frame, 2).await, frame);
    }

    #[tokio::test]
    async fn roundtrip_variable_empty_data() {
        let frame = LinkFrame::Variable {
            control: ControlField::Secondary {
                acd: true,
                dfc: true,
                function: SecondaryFunction::NoData,
            },
            address: LinkAddress(0),
            data: Bytes::new(),
        };
        assert_eq!(write_and_read(&frame, 1).await, frame);
    }

    #[tokio::test]
    async fn invalid_start_byte() {
        let (mut client, mut server) = duplex(64);
        client.write_all(&[0x99]).await.unwrap();
        client.flush().await.unwrap();
        let mut parser = LinkFrameParser::new(1);
        let err = parser.read_frame(&mut server).await.unwrap_err();
        assert!(matches!(err, LinkError::InvalidStartByte(0x99)));
    }

    #[tokio::test]
    async fn checksum_verification() {
        let (mut client, mut server) = duplex(64);
        let cf = ControlField::Primary {
            fcb: false,
            fcv: false,
            function: PrimaryFunction::ResetLink,
        }
        .to_byte();
        let addr: u8 = 1;
        let bad_cs: u8 = 0xFF;
        client
            .write_all(&[START_FIXED, cf, addr, bad_cs, END_BYTE])
            .await
            .unwrap();
        client.flush().await.unwrap();
        let mut parser = LinkFrameParser::new(1);
        let err = parser.read_frame(&mut server).await.unwrap_err();
        assert!(matches!(err, LinkError::ChecksumMismatch { .. }));
    }

    #[tokio::test]
    async fn multiple_frames_in_buffer() {
        let (mut client, mut server) = duplex(256);
        let f1 = LinkFrame::SingleAck;
        let f2 = LinkFrame::Fixed {
            control: ControlField::Secondary {
                acd: false,
                dfc: false,
                function: SecondaryFunction::Ack,
            },
            address: LinkAddress(1),
        };
        write_link_frame(&mut client, &f1, 1).await.unwrap();
        write_link_frame(&mut client, &f2, 1).await.unwrap();

        let mut parser = LinkFrameParser::new(1);
        assert_eq!(parser.read_frame(&mut server).await.unwrap(), f1);
        assert_eq!(parser.read_frame(&mut server).await.unwrap(), f2);
    }

    #[test]
    fn control_field_roundtrip() {
        let cases = vec![
            ControlField::Primary {
                fcb: false,
                fcv: false,
                function: PrimaryFunction::ResetLink,
            },
            ControlField::Primary {
                fcb: true,
                fcv: true,
                function: PrimaryFunction::SendConfirm,
            },
            ControlField::Primary {
                fcb: true,
                fcv: true,
                function: PrimaryFunction::RequestClass1,
            },
            ControlField::Secondary {
                acd: true,
                dfc: false,
                function: SecondaryFunction::Ack,
            },
            ControlField::Secondary {
                acd: false,
                dfc: true,
                function: SecondaryFunction::UserData,
            },
        ];
        for cf in cases {
            let byte = cf.to_byte();
            let parsed = ControlField::from_byte(byte).unwrap();
            assert_eq!(cf, parsed, "roundtrip failed for byte 0x{:02X}", byte);
        }
    }

    #[tokio::test]
    async fn try_parse_returns_none_on_empty() {
        let mut parser = LinkFrameParser::new(1);
        assert!(parser.try_parse().unwrap().is_none());
    }

    #[tokio::test]
    async fn try_parse_returns_none_on_partial_fixed() {
        let mut parser = LinkFrameParser::new(1);
        parser.buf.extend_from_slice(&[START_FIXED, 0x40]);
        assert!(parser.try_parse().unwrap().is_none());
        assert_eq!(parser.buf.len(), 2);
    }

    #[tokio::test]
    async fn try_parse_returns_none_on_partial_variable() {
        let mut parser = LinkFrameParser::new(1);
        parser
            .buf
            .extend_from_slice(&[START_VARIABLE, 0x05, 0x05, START_VARIABLE]);
        assert!(parser.try_parse().unwrap().is_none());
        assert_eq!(parser.buf.len(), 4);
    }
}
