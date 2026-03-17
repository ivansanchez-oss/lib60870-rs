use crate::error::{Error, Result};
use crate::types::CauseOfTransmission;
use crate::info::InformationObject;

use super::header::AsduHeader;
use super::ioa::InformationObjectAddress;
use super::{AddressedObject, Asdu};

/// Fluent builder for constructing validated ASDUs.
pub struct AsduBuilder {
    cause: CauseOfTransmission,
    common_address: u16,
    is_test: bool,
    is_negative: bool,
    originator_address: u8,
    is_sequence: bool,
    objects: Vec<AddressedObject>,
}

impl AsduBuilder {
    pub fn new(cause: CauseOfTransmission, common_address: u16) -> Self {
        Self {
            cause,
            common_address,
            is_test: false,
            is_negative: false,
            originator_address: 0,
            is_sequence: false,
            objects: Vec::new(),
        }
    }

    pub fn test(mut self, value: bool) -> Self {
        self.is_test = value;
        self
    }

    pub fn negative(mut self, value: bool) -> Self {
        self.is_negative = value;
        self
    }

    pub fn originator(mut self, addr: u8) -> Self {
        self.originator_address = addr;
        self
    }

    pub fn sequential(mut self, value: bool) -> Self {
        self.is_sequence = value;
        self
    }

    /// Add an information object. All objects must share the same TypeId.
    pub fn add(
        mut self,
        ioa: impl Into<InformationObjectAddress>,
        object: InformationObject,
    ) -> Result<Self> {
        if self.objects.len() >= 127 {
            return Err(Error::Encode(
                "ASDU cannot contain more than 127 objects".into(),
            ));
        }

        if let Some(first) = self.objects.first() {
            if first.object.type_id() != object.type_id() {
                return Err(Error::Encode(format!(
                    "type mismatch: expected {}, got {}",
                    first.object.type_id(),
                    object.type_id()
                )));
            }
        }

        self.objects.push(AddressedObject {
            address: ioa.into(),
            object,
        });
        Ok(self)
    }

    /// Build the ASDU. Fails if no objects have been added.
    pub fn build(self) -> Result<Asdu> {
        if self.objects.is_empty() {
            return Err(Error::Encode("ASDU must contain at least one object".into()));
        }

        let type_id = self.objects[0].object.type_id();

        Ok(Asdu {
            header: AsduHeader {
                type_id,
                is_sequence: self.is_sequence,
                num_objects: self.objects.len() as u8,
                cause: self.cause,
                is_test: self.is_test,
                is_negative: self.is_negative,
                originator_address: self.originator_address,
                common_address: self.common_address,
            },
            objects: self.objects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::info::SinglePointInformation;
    use crate::types::QualityDescriptor;

    fn spi(val: bool) -> InformationObject {
        InformationObject::SinglePoint(SinglePointInformation::new(val, QualityDescriptor::empty()))
    }

    #[test]
    fn build_simple() {
        let asdu = AsduBuilder::new(CauseOfTransmission::Spontaneous, 1)
            .add(100u32, spi(true))
            .unwrap()
            .add(101u32, spi(false))
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(asdu.header.num_objects, 2);
        assert_eq!(asdu.header.type_id, crate::types::TypeId::MSpNa1);
        assert_eq!(asdu.objects[0].address.value(), 100);
        assert_eq!(asdu.objects[1].address.value(), 101);
    }

    #[test]
    fn build_sequential() {
        let asdu = AsduBuilder::new(CauseOfTransmission::Spontaneous, 1)
            .sequential(true)
            .add(100u32, spi(true))
            .unwrap()
            .add(101u32, spi(false))
            .unwrap()
            .build()
            .unwrap();

        assert!(asdu.header.is_sequence);
    }

    #[test]
    fn type_mismatch() {
        let result = AsduBuilder::new(CauseOfTransmission::Spontaneous, 1)
            .add(100u32, spi(true))
            .unwrap()
            .add(
                101u32,
                InformationObject::Interrogation(
                    crate::info::InterrogationCommand::station(),
                ),
            );

        assert!(result.is_err());
    }

    #[test]
    fn build_empty() {
        let result = AsduBuilder::new(CauseOfTransmission::Spontaneous, 1).build();
        assert!(result.is_err());
    }

    #[test]
    fn overflow_127() {
        let mut builder = AsduBuilder::new(CauseOfTransmission::Spontaneous, 1);
        for i in 0..127u32 {
            builder = builder.add(i, spi(true)).unwrap();
        }
        let result = builder.add(127u32, spi(true));
        assert!(result.is_err());
    }

    #[test]
    fn builder_flags() {
        let asdu = AsduBuilder::new(CauseOfTransmission::Activation, 0x1234)
            .test(true)
            .negative(true)
            .originator(42)
            .add(1u32, spi(true))
            .unwrap()
            .build()
            .unwrap();

        assert!(asdu.header.is_test);
        assert!(asdu.header.is_negative);
        assert_eq!(asdu.header.originator_address, 42);
        assert_eq!(asdu.header.common_address, 0x1234);
    }
}
