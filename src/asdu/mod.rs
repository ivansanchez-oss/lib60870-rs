pub mod ioa;
pub mod header;
pub mod builder;

use bytes::{Buf, BufMut};

use crate::error::{Error, Result};
use crate::info::InformationObject;
use crate::types::AppLayerParameters;

pub use builder::AsduBuilder;
pub use header::AsduHeader;
pub use ioa::InformationObjectAddress;

/// An information object paired with its address.
///
/// Generic over the object type `T`, allowing use with both the
/// [`InformationObject`] enum and specific typed objects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Indexed<T> {
    pub address: InformationObjectAddress,
    pub value: T,
}

/// Application Service Data Unit — header plus a collection of addressed
/// information objects.
#[derive(Debug, Clone, PartialEq)]
pub struct Asdu {
    pub header: AsduHeader,
    pub objects: Vec<Indexed<InformationObject>>,
}

impl Asdu {
    /// Decode a complete ASDU from the buffer.
    pub fn decode(buf: &mut impl Buf, params: &AppLayerParameters) -> Result<Self> {
        let header = AsduHeader::decode(buf, params)?;
        let n = header.num_objects as usize;
        let mut objects = Vec::with_capacity(n);

        if header.is_sequence {
            // SQ=1: first object has an IOA, subsequent IOAs are sequential
            if n == 0 {
                return Ok(Self { header, objects });
            }
            let base_ioa = InformationObjectAddress::decode(buf, params.size_of_ioa())?;
            let first = InformationObject::decode(header.type_id, buf)?;
            objects.push(Indexed {
                address: base_ioa,
                value: first,
            });
            for i in 1..n {
                let ioa =
                    InformationObjectAddress::try_new(base_ioa.value() + i as u32)?;
                let obj = InformationObject::decode(header.type_id, buf)?;
                objects.push(Indexed {
                    address: ioa,
                    value: obj,
                });
            }
        } else {
            // SQ=0: each object has its own IOA
            for _ in 0..n {
                let ioa = InformationObjectAddress::decode(buf, params.size_of_ioa())?;
                let obj = InformationObject::decode(header.type_id, buf)?;
                objects.push(Indexed {
                    address: ioa,
                    value: obj,
                });
            }
        }

        Ok(Self { header, objects })
    }

    /// Encode the ASDU into the buffer.
    pub fn encode(&self, buf: &mut impl BufMut, params: &AppLayerParameters) -> Result<()> {
        // Validate: all objects must have the same type_id as the header
        for ao in &self.objects {
            if ao.value.type_id() != self.header.type_id {
                return Err(Error::Encode(format!(
                    "object type {} does not match header type {}",
                    ao.value.type_id(),
                    self.header.type_id
                )));
            }
        }

        // Validate: total encoded size must not exceed max_asdu_length
        let total = self.encoded_size(params);
        if total > params.max_asdu_length() as usize {
            return Err(Error::Encode(format!(
                "ASDU size {} exceeds max_asdu_length {}",
                total, params.max_asdu_length()
            )));
        }

        // Validate: if SQ=1, IOAs must be consecutive
        if self.header.is_sequence && self.objects.len() > 1 {
            let base = self.objects[0].address.value();
            for (i, ao) in self.objects.iter().enumerate().skip(1) {
                let expected = base + i as u32;
                if ao.address.value() != expected {
                    return Err(Error::Encode(format!(
                        "sequential IOA at index {}: expected {}, got {}",
                        i, expected, ao.address.value()
                    )));
                }
            }
        }

        self.header.encode(buf, params)?;

        if self.header.is_sequence {
            // SQ=1: write first IOA, then all payloads without IOAs
            if let Some(first) = self.objects.first() {
                first.address.encode(buf, params.size_of_ioa())?;
                for ao in &self.objects {
                    ao.value.encode(buf)?;
                }
            }
        } else {
            // SQ=0: write IOA + payload for each object
            for ao in &self.objects {
                ao.address.encode(buf, params.size_of_ioa())?;
                ao.value.encode(buf)?;
            }
        }

        Ok(())
    }

    /// Calculate the total encoded size in bytes.
    pub fn encoded_size(&self, params: &AppLayerParameters) -> usize {
        let header_size = AsduHeader::encoded_size(params);
        let ioa_size = params.size_of_ioa() as usize;

        if self.objects.is_empty() {
            return header_size;
        }

        let obj_data_size: usize = self.objects.iter().map(|ao| ao.value.encoded_size()).sum();

        if self.header.is_sequence {
            // SQ=1: one IOA + N payloads
            header_size + ioa_size + obj_data_size
        } else {
            // SQ=0: N * (IOA + payload)
            header_size + self.objects.len() * ioa_size + obj_data_size
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};
    use crate::info::SinglePointInformation;
    use crate::types::{CauseOfTransmission, CommonAddress, OriginatorAddress, QualityDescriptor, TypeId};

    fn spi(val: bool) -> InformationObject {
        InformationObject::SinglePoint(SinglePointInformation::new(val, QualityDescriptor::empty()))
    }

    fn cs104() -> AppLayerParameters {
        AppLayerParameters::CS104_DEFAULT
    }

    #[test]
    fn roundtrip_sq0() {
        let asdu = Asdu {
            header: AsduHeader {
                type_id: TypeId::MSpNa1,
                is_sequence: false,
                num_objects: 3,
                cause: CauseOfTransmission::Spontaneous,
                is_test: false,
                is_negative: false,
                originator_address: OriginatorAddress::default(),
                common_address: CommonAddress::new(1),
            },
            objects: vec![
                Indexed { address: 100u16.into(), value: spi(true) },
                Indexed { address: 200u16.into(), value: spi(false) },
                Indexed { address: 300u16.into(), value: spi(true) },
            ],
        };

        let params = cs104();
        let mut buf = BytesMut::with_capacity(64);
        asdu.encode(&mut buf, &params).unwrap();
        assert_eq!(buf.len(), asdu.encoded_size(&params));

        let mut reader = Bytes::from(buf);
        let decoded = Asdu::decode(&mut reader, &params).unwrap();
        assert_eq!(asdu, decoded);
    }

    #[test]
    fn roundtrip_sq1() {
        let asdu = Asdu {
            header: AsduHeader {
                type_id: TypeId::MSpNa1,
                is_sequence: true,
                num_objects: 3,
                cause: CauseOfTransmission::InterrogatedByStation,
                is_test: false,
                is_negative: false,
                originator_address: OriginatorAddress::default(),
                common_address: CommonAddress::new(1),
            },
            objects: vec![
                Indexed { address: 100u16.into(), value: spi(true) },
                Indexed { address: 101u16.into(), value: spi(false) },
                Indexed { address: 102u16.into(), value: spi(true) },
            ],
        };

        let params = cs104();
        let mut buf = BytesMut::with_capacity(64);
        asdu.encode(&mut buf, &params).unwrap();

        // SQ=1 should be smaller: 1 IOA instead of 3
        let sq0_size = AsduHeader::encoded_size(&params) + 3 * (3 + 1); // 3 * (ioa + spi)
        let sq1_size = asdu.encoded_size(&params);
        assert!(sq1_size < sq0_size);

        let mut reader = Bytes::from(buf);
        let decoded = Asdu::decode(&mut reader, &params).unwrap();
        assert_eq!(asdu, decoded);
    }

    #[test]
    fn encode_rejects_heterogeneous_types() {
        let asdu = Asdu {
            header: AsduHeader {
                type_id: TypeId::MSpNa1,
                is_sequence: false,
                num_objects: 2,
                cause: CauseOfTransmission::Spontaneous,
                is_test: false,
                is_negative: false,
                originator_address: OriginatorAddress::default(),
                common_address: CommonAddress::new(1),
            },
            objects: vec![
                Indexed { address: 100u16.into(), value: spi(true) },
                Indexed {
                    address: 101u16.into(),
                    value: InformationObject::Interrogation(
                        crate::info::InterrogationCommand::station(),
                    ),
                },
            ],
        };

        let params = cs104();
        let mut buf = BytesMut::with_capacity(64);
        assert!(asdu.encode(&mut buf, &params).is_err());
    }

    #[test]
    fn encode_rejects_non_sequential_ioas_with_sq1() {
        let asdu = Asdu {
            header: AsduHeader {
                type_id: TypeId::MSpNa1,
                is_sequence: true,
                num_objects: 2,
                cause: CauseOfTransmission::Spontaneous,
                is_test: false,
                is_negative: false,
                originator_address: OriginatorAddress::default(),
                common_address: CommonAddress::new(1),
            },
            objects: vec![
                Indexed { address: 100u16.into(), value: spi(true) },
                Indexed { address: 200u16.into(), value: spi(false) }, // gap!
            ],
        };

        let params = cs104();
        let mut buf = BytesMut::with_capacity(64);
        assert!(asdu.encode(&mut buf, &params).is_err());
    }

    #[test]
    fn encode_rejects_exceeding_max_length() {
        let params = AppLayerParameters::builder()
            .max_asdu_length(10) // artificially small
            .build()
            .unwrap();

        let asdu = AsduBuilder::new(CauseOfTransmission::Spontaneous, CommonAddress::new(1))
            .add(100u16, spi(true))
            .unwrap()
            .add(101u16, spi(false))
            .unwrap()
            .add(102u16, spi(true))
            .unwrap()
            .build()
            .unwrap();

        let mut buf = BytesMut::with_capacity(64);
        assert!(asdu.encode(&mut buf, &params).is_err());
    }

    #[test]
    fn builder_encode_roundtrip() {
        let params = cs104();
        let asdu = AsduBuilder::new(CauseOfTransmission::Spontaneous, CommonAddress::new(1))
            .originator(OriginatorAddress::new(7))
            .sequential(true)
            .add(100u16, spi(true))
            .unwrap()
            .add(101u16, spi(false))
            .unwrap()
            .build()
            .unwrap();

        let mut buf = BytesMut::with_capacity(64);
        asdu.encode(&mut buf, &params).unwrap();

        let mut reader = Bytes::from(buf);
        let decoded = Asdu::decode(&mut reader, &params).unwrap();
        assert_eq!(asdu, decoded);
    }

    #[test]
    fn encoded_size_empty() {
        let asdu = Asdu {
            header: AsduHeader {
                type_id: TypeId::MSpNa1,
                is_sequence: false,
                num_objects: 0,
                cause: CauseOfTransmission::Spontaneous,
                is_test: false,
                is_negative: false,
                originator_address: OriginatorAddress::default(),
                common_address: CommonAddress::new(1),
            },
            objects: vec![],
        };
        let params = cs104();
        assert_eq!(asdu.encoded_size(&params), params.asdu_header_size());
    }

    #[test]
    fn roundtrip_cs101_small_params() {
        let params = AppLayerParameters::builder()
            .size_of_cot(1)
            .size_of_ca(1)
            .size_of_ioa(1)
            .max_asdu_length(254)
            .build()
            .unwrap();

        let asdu = Asdu {
            header: AsduHeader {
                type_id: TypeId::MSpNa1,
                is_sequence: false,
                num_objects: 2,
                cause: CauseOfTransmission::Spontaneous,
                is_test: false,
                is_negative: false,
                originator_address: OriginatorAddress::default(),
                common_address: CommonAddress::new(5),
            },
            objects: vec![
                Indexed { address: 10u16.into(), value: spi(true) },
                Indexed { address: 20u16.into(), value: spi(false) },
            ],
        };

        let mut buf = BytesMut::with_capacity(64);
        asdu.encode(&mut buf, &params).unwrap();
        assert_eq!(buf.len(), asdu.encoded_size(&params));

        let mut reader = Bytes::from(buf);
        let decoded = Asdu::decode(&mut reader, &params).unwrap();
        assert_eq!(asdu, decoded);
    }
}
