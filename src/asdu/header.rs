use bytes::{Buf, BufMut};

use crate::error::{Error, Result};
use crate::types::{AppLayerParameters, CauseOfTransmission, CommonAddress, OriginatorAddress, TypeId};

/// ASDU header: type identification, variable structure qualifier, cause of
/// transmission, and common address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsduHeader {
    pub type_id: TypeId,
    /// SQ flag — when true, information objects have sequential IOAs.
    pub is_sequence: bool,
    /// Number of information objects (0-127).
    pub num_objects: u8,
    pub cause: CauseOfTransmission,
    /// Test bit from the COT byte.
    pub is_test: bool,
    /// Positive/Negative confirmation bit from the COT byte.
    pub is_negative: bool,
    /// Originator address (byte 1 of COT when `size_of_cot == 2`).
    pub originator_address: OriginatorAddress,
    /// Common address of ASDU (1-2 bytes depending on `size_of_ca`).
    pub common_address: CommonAddress,
}

impl AsduHeader {
    /// Encoded header size for the given parameters.
    pub fn encoded_size(params: &AppLayerParameters) -> usize {
        params.asdu_header_size()
    }

    /// Decode an ASDU header from the buffer.
    pub fn decode(buf: &mut impl Buf, params: &AppLayerParameters) -> Result<Self> {
        let need = params.asdu_header_size();
        if buf.remaining() < need {
            return Err(Error::BufferTooShort {
                need,
                have: buf.remaining(),
            });
        }

        // Type ID (1 byte)
        let type_id_raw = buf.get_u8();
        let type_id = TypeId::try_from(type_id_raw).map_err(|_| Error::InvalidValue {
            type_name: "TypeId",
            value: type_id_raw,
        })?;

        // VSQ (1 byte): bit 7 = SQ, bits 0-6 = number of objects
        let vsq = buf.get_u8();
        let is_sequence = (vsq & 0x80) != 0;
        let num_objects = vsq & 0x7F;

        // COT byte 0: bits 0-5 = cause, bit 6 = P/N, bit 7 = T
        let cot_byte0 = buf.get_u8();
        let cause_raw = cot_byte0 & 0x3F;
        let is_negative = (cot_byte0 & 0x40) != 0;
        let is_test = (cot_byte0 & 0x80) != 0;
        let cause =
            CauseOfTransmission::try_from(cause_raw).map_err(|_| Error::InvalidValue {
                type_name: "CauseOfTransmission",
                value: cause_raw,
            })?;

        // COT byte 1 (originator address, if size_of_cot == 2)
        let originator_address = if params.size_of_cot() >= 2 {
            OriginatorAddress::new(buf.get_u8())
        } else {
            OriginatorAddress::default()
        };

        // Common address (1-2 bytes LE)
        let common_address = if params.size_of_ca() >= 2 {
            CommonAddress::new(buf.get_u16_le())
        } else {
            CommonAddress::new(buf.get_u8() as u16)
        };

        Ok(Self {
            type_id,
            is_sequence,
            num_objects,
            cause,
            is_test,
            is_negative,
            originator_address,
            common_address,
        })
    }

    /// Encode the ASDU header into the buffer.
    pub fn encode(&self, buf: &mut impl BufMut, params: &AppLayerParameters) -> Result<()> {
        let need = params.asdu_header_size();
        if buf.remaining_mut() < need {
            return Err(Error::BufferTooShort {
                need,
                have: buf.remaining_mut(),
            });
        }

        if self.num_objects > 127 {
            return Err(Error::Encode(format!(
                "num_objects {} exceeds maximum 127",
                self.num_objects
            )));
        }

        // Type ID
        buf.put_u8(self.type_id.as_u8());

        // VSQ
        let vsq = if self.is_sequence { 0x80 } else { 0 } | self.num_objects;
        buf.put_u8(vsq);

        // COT byte 0
        let cot_byte0 = (self.cause.as_u8() & 0x3F)
            | if self.is_negative { 0x40 } else { 0 }
            | if self.is_test { 0x80 } else { 0 };
        buf.put_u8(cot_byte0);

        // COT byte 1
        if params.size_of_cot() >= 2 {
            buf.put_u8(self.originator_address.value());
        }

        // Common address
        if params.size_of_ca() >= 2 {
            buf.put_u16_le(self.common_address.value());
        } else {
            buf.put_u8(self.common_address.value() as u8);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};

    #[test]
    fn roundtrip_cs104() {
        let params = AppLayerParameters::CS104_DEFAULT;
        let header = AsduHeader {
            type_id: TypeId::MSpNa1,
            is_sequence: false,
            num_objects: 3,
            cause: CauseOfTransmission::Spontaneous,
            is_test: false,
            is_negative: false,
            originator_address: OriginatorAddress::default(),
            common_address: CommonAddress::new(1),
        };

        let mut buf = BytesMut::with_capacity(32);
        header.encode(&mut buf, &params).unwrap();
        assert_eq!(buf.len(), params.asdu_header_size());

        let mut reader = Bytes::from(buf);
        let decoded = AsduHeader::decode(&mut reader, &params).unwrap();
        assert_eq!(header, decoded);
    }

    #[test]
    fn roundtrip_cs101_1byte_ca() {
        let params = AppLayerParameters::builder()
            .size_of_cot(1)
            .size_of_ca(1)
            .size_of_ioa(2)
            .max_asdu_length(254)
            .build()
            .unwrap();
        let header = AsduHeader {
            type_id: TypeId::CScNa1,
            is_sequence: true,
            num_objects: 1,
            cause: CauseOfTransmission::Activation,
            is_test: true,
            is_negative: true,
            originator_address: OriginatorAddress::default(), // ignored when size_of_cot == 1
            common_address: CommonAddress::new(5),
        };

        let mut buf = BytesMut::with_capacity(32);
        header.encode(&mut buf, &params).unwrap();
        // type_id(1) + vsq(1) + cot(1) + ca(1) = 4
        assert_eq!(buf.len(), 4);

        let mut reader = Bytes::from(buf);
        let decoded = AsduHeader::decode(&mut reader, &params).unwrap();
        assert_eq!(decoded.type_id, header.type_id);
        assert_eq!(decoded.is_sequence, header.is_sequence);
        assert_eq!(decoded.cause, header.cause);
        assert_eq!(decoded.is_test, header.is_test);
        assert_eq!(decoded.is_negative, header.is_negative);
        assert_eq!(decoded.common_address, header.common_address);
        // originator_address not encoded when size_of_cot == 1
        assert_eq!(decoded.originator_address, OriginatorAddress::default());
    }

    #[test]
    fn test_and_negative_bits() {
        let params = AppLayerParameters::CS104_DEFAULT;
        let header = AsduHeader {
            type_id: TypeId::MSpNa1,
            is_sequence: false,
            num_objects: 1,
            cause: CauseOfTransmission::Activation,
            is_test: true,
            is_negative: true,
            originator_address: OriginatorAddress::new(42),
            common_address: CommonAddress::new(0x1234),
        };

        let mut buf = BytesMut::with_capacity(32);
        header.encode(&mut buf, &params).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = AsduHeader::decode(&mut reader, &params).unwrap();
        assert_eq!(decoded.is_test, true);
        assert_eq!(decoded.is_negative, true);
        assert_eq!(decoded.originator_address, OriginatorAddress::new(42));
        assert_eq!(decoded.common_address, CommonAddress::new(0x1234));
    }

    #[test]
    fn sequence_flag() {
        let params = AppLayerParameters::CS104_DEFAULT;
        let header = AsduHeader {
            type_id: TypeId::MSpNa1,
            is_sequence: true,
            num_objects: 127,
            cause: CauseOfTransmission::Periodic,
            is_test: false,
            is_negative: false,
            originator_address: OriginatorAddress::default(),
            common_address: CommonAddress::new(1),
        };

        let mut buf = BytesMut::with_capacity(32);
        header.encode(&mut buf, &params).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = AsduHeader::decode(&mut reader, &params).unwrap();
        assert_eq!(decoded.is_sequence, true);
        assert_eq!(decoded.num_objects, 127);
    }

    #[test]
    fn buffer_too_short() {
        let params = AppLayerParameters::CS104_DEFAULT;
        let mut buf = Bytes::from_static(&[0x01, 0x02]);
        assert!(AsduHeader::decode(&mut buf, &params).is_err());
    }
}
