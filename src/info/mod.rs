pub mod traits;
pub mod single_point;
pub mod double_point;
pub mod measured_normalized;
pub mod measured_scaled;
pub mod measured_short;
pub mod integrated_totals;
pub mod commands;
pub mod system;

use bytes::{Buf, BufMut};

use crate::error::{Error, Result};
use crate::types::{Cp56Time2a, TypeId};

pub use traits::{Decode, Encode};
pub use single_point::SinglePointInformation;
pub use double_point::DoublePointInformation;
pub use measured_normalized::{MeasuredValueNormalized, MeasuredValueNormalizedNoQuality};
pub use measured_scaled::MeasuredValueScaled;
pub use measured_short::MeasuredValueShortFloat;
pub use integrated_totals::BinaryCounterReading;
pub use commands::SingleCommand;
pub use system::{
    InterrogationCommand, CounterInterrogationCommand, ReadCommand,
    ClockSyncCommand, EndOfInitialization,
};

/// The payload of an information object — variant per TypeId.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InformationObject {
    // Monitoring — without time tag
    SinglePoint(SinglePointInformation),
    DoublePoint(DoublePointInformation),
    MeasuredNormalized(MeasuredValueNormalized),
    MeasuredNormalizedNoQuality(MeasuredValueNormalizedNoQuality),
    MeasuredScaled(MeasuredValueScaled),
    MeasuredShortFloat(MeasuredValueShortFloat),
    IntegratedTotals(BinaryCounterReading),

    // Monitoring — with CP56Time2a
    SinglePointCp56(SinglePointInformation, Cp56Time2a),
    DoublePointCp56(DoublePointInformation, Cp56Time2a),
    MeasuredNormalizedCp56(MeasuredValueNormalized, Cp56Time2a),
    MeasuredScaledCp56(MeasuredValueScaled, Cp56Time2a),
    MeasuredShortFloatCp56(MeasuredValueShortFloat, Cp56Time2a),
    IntegratedTotalsCp56(BinaryCounterReading, Cp56Time2a),

    // Commands
    SingleCommand(SingleCommand),
    SingleCommandCp56(SingleCommand, Cp56Time2a),

    // System
    Interrogation(InterrogationCommand),
    CounterInterrogation(CounterInterrogationCommand),
    Read(ReadCommand),
    ClockSync(ClockSyncCommand),
    EndOfInit(EndOfInitialization),
}

impl InformationObject {
    /// Decode the payload for a given TypeId.
    pub fn decode(type_id: TypeId, buf: &mut impl Buf) -> Result<Self> {
        match type_id {
            // Single point
            TypeId::MSpNa1 => Ok(Self::SinglePoint(SinglePointInformation::decode(buf)?)),
            TypeId::MSpTb1 => {
                let spi = SinglePointInformation::decode(buf)?;
                let time = decode_cp56(buf)?;
                Ok(Self::SinglePointCp56(spi, time))
            }
            // Double point
            TypeId::MDpNa1 => Ok(Self::DoublePoint(DoublePointInformation::decode(buf)?)),
            TypeId::MDpTb1 => {
                let dpi = DoublePointInformation::decode(buf)?;
                let time = decode_cp56(buf)?;
                Ok(Self::DoublePointCp56(dpi, time))
            }
            // Measured normalized
            TypeId::MMeNa1 => Ok(Self::MeasuredNormalized(MeasuredValueNormalized::decode(buf)?)),
            TypeId::MMeNd1 => Ok(Self::MeasuredNormalizedNoQuality(MeasuredValueNormalizedNoQuality::decode(buf)?)),
            TypeId::MMeTd1 => {
                let mv = MeasuredValueNormalized::decode(buf)?;
                let time = decode_cp56(buf)?;
                Ok(Self::MeasuredNormalizedCp56(mv, time))
            }
            // Measured scaled
            TypeId::MMeNb1 => Ok(Self::MeasuredScaled(MeasuredValueScaled::decode(buf)?)),
            TypeId::MMeTe1 => {
                let mv = MeasuredValueScaled::decode(buf)?;
                let time = decode_cp56(buf)?;
                Ok(Self::MeasuredScaledCp56(mv, time))
            }
            // Measured short float
            TypeId::MMeNc1 => Ok(Self::MeasuredShortFloat(MeasuredValueShortFloat::decode(buf)?)),
            TypeId::MMeTf1 => {
                let mv = MeasuredValueShortFloat::decode(buf)?;
                let time = decode_cp56(buf)?;
                Ok(Self::MeasuredShortFloatCp56(mv, time))
            }
            // Integrated totals
            TypeId::MItNa1 => Ok(Self::IntegratedTotals(BinaryCounterReading::decode(buf)?)),
            TypeId::MItTb1 => {
                let bcr = BinaryCounterReading::decode(buf)?;
                let time = decode_cp56(buf)?;
                Ok(Self::IntegratedTotalsCp56(bcr, time))
            }
            // Commands
            TypeId::CScNa1 => Ok(Self::SingleCommand(SingleCommand::decode(buf)?)),
            TypeId::CScTa1 => {
                let cmd = SingleCommand::decode(buf)?;
                let time = decode_cp56(buf)?;
                Ok(Self::SingleCommandCp56(cmd, time))
            }
            // System
            TypeId::CIcNa1 => Ok(Self::Interrogation(InterrogationCommand::decode(buf)?)),
            TypeId::CCiNa1 => Ok(Self::CounterInterrogation(CounterInterrogationCommand::decode(buf)?)),
            TypeId::CRdNa1 => Ok(Self::Read(ReadCommand::decode(buf)?)),
            TypeId::CCsNa1 => Ok(Self::ClockSync(ClockSyncCommand::decode(buf)?)),
            TypeId::MEiNa1 => Ok(Self::EndOfInit(EndOfInitialization::decode(buf)?)),
            _ => Err(Error::Decode(format!("unsupported type id: {type_id}"))),
        }
    }

    /// Returns the TypeId corresponding to this variant.
    pub fn type_id(&self) -> TypeId {
        match self {
            Self::SinglePoint(_) => TypeId::MSpNa1,
            Self::SinglePointCp56(_, _) => TypeId::MSpTb1,
            Self::DoublePoint(_) => TypeId::MDpNa1,
            Self::DoublePointCp56(_, _) => TypeId::MDpTb1,
            Self::MeasuredNormalized(_) => TypeId::MMeNa1,
            Self::MeasuredNormalizedNoQuality(_) => TypeId::MMeNd1,
            Self::MeasuredNormalizedCp56(_, _) => TypeId::MMeTd1,
            Self::MeasuredScaled(_) => TypeId::MMeNb1,
            Self::MeasuredScaledCp56(_, _) => TypeId::MMeTe1,
            Self::MeasuredShortFloat(_) => TypeId::MMeNc1,
            Self::MeasuredShortFloatCp56(_, _) => TypeId::MMeTf1,
            Self::IntegratedTotals(_) => TypeId::MItNa1,
            Self::IntegratedTotalsCp56(_, _) => TypeId::MItTb1,
            Self::SingleCommand(_) => TypeId::CScNa1,
            Self::SingleCommandCp56(_, _) => TypeId::CScTa1,
            Self::Interrogation(_) => TypeId::CIcNa1,
            Self::CounterInterrogation(_) => TypeId::CCiNa1,
            Self::Read(_) => TypeId::CRdNa1,
            Self::ClockSync(_) => TypeId::CCsNa1,
            Self::EndOfInit(_) => TypeId::MEiNa1,
        }
    }

    pub fn encode(&self, buf: &mut impl BufMut) -> Result<()> {
        match self {
            Self::SinglePoint(v) => v.encode(buf),
            Self::SinglePointCp56(v, t) => { v.encode(buf)?; encode_cp56(t, buf) }
            Self::DoublePoint(v) => v.encode(buf),
            Self::DoublePointCp56(v, t) => { v.encode(buf)?; encode_cp56(t, buf) }
            Self::MeasuredNormalized(v) => v.encode(buf),
            Self::MeasuredNormalizedNoQuality(v) => v.encode(buf),
            Self::MeasuredNormalizedCp56(v, t) => { v.encode(buf)?; encode_cp56(t, buf) }
            Self::MeasuredScaled(v) => v.encode(buf),
            Self::MeasuredScaledCp56(v, t) => { v.encode(buf)?; encode_cp56(t, buf) }
            Self::MeasuredShortFloat(v) => v.encode(buf),
            Self::MeasuredShortFloatCp56(v, t) => { v.encode(buf)?; encode_cp56(t, buf) }
            Self::IntegratedTotals(v) => v.encode(buf),
            Self::IntegratedTotalsCp56(v, t) => { v.encode(buf)?; encode_cp56(t, buf) }
            Self::SingleCommand(v) => v.encode(buf),
            Self::SingleCommandCp56(v, t) => { v.encode(buf)?; encode_cp56(t, buf) }
            Self::Interrogation(v) => v.encode(buf),
            Self::CounterInterrogation(v) => v.encode(buf),
            Self::Read(v) => v.encode(buf),
            Self::ClockSync(v) => v.encode(buf),
            Self::EndOfInit(v) => v.encode(buf),
        }
    }

    pub fn encoded_size(&self) -> usize {
        match self {
            Self::SinglePoint(v) => v.encoded_size(),
            Self::SinglePointCp56(v, _) => v.encoded_size() + Cp56Time2a::ENCODED_SIZE,
            Self::DoublePoint(v) => v.encoded_size(),
            Self::DoublePointCp56(v, _) => v.encoded_size() + Cp56Time2a::ENCODED_SIZE,
            Self::MeasuredNormalized(v) => v.encoded_size(),
            Self::MeasuredNormalizedNoQuality(v) => v.encoded_size(),
            Self::MeasuredNormalizedCp56(v, _) => v.encoded_size() + Cp56Time2a::ENCODED_SIZE,
            Self::MeasuredScaled(v) => v.encoded_size(),
            Self::MeasuredScaledCp56(v, _) => v.encoded_size() + Cp56Time2a::ENCODED_SIZE,
            Self::MeasuredShortFloat(v) => v.encoded_size(),
            Self::MeasuredShortFloatCp56(v, _) => v.encoded_size() + Cp56Time2a::ENCODED_SIZE,
            Self::IntegratedTotals(v) => v.encoded_size(),
            Self::IntegratedTotalsCp56(v, _) => v.encoded_size() + Cp56Time2a::ENCODED_SIZE,
            Self::SingleCommand(v) => v.encoded_size(),
            Self::SingleCommandCp56(v, _) => v.encoded_size() + Cp56Time2a::ENCODED_SIZE,
            Self::Interrogation(v) => v.encoded_size(),
            Self::CounterInterrogation(v) => v.encoded_size(),
            Self::Read(v) => v.encoded_size(),
            Self::ClockSync(v) => v.encoded_size(),
            Self::EndOfInit(v) => v.encoded_size(),
        }
    }
}

fn decode_cp56(buf: &mut impl Buf) -> Result<Cp56Time2a> {
    if buf.remaining() < Cp56Time2a::ENCODED_SIZE {
        return Err(Error::BufferTooShort {
            need: Cp56Time2a::ENCODED_SIZE,
            have: buf.remaining(),
        });
    }
    let mut bytes = [0u8; 7];
    buf.copy_to_slice(&mut bytes);
    Cp56Time2a::from_bytes(&bytes)
}

fn encode_cp56(time: &Cp56Time2a, buf: &mut impl BufMut) -> Result<()> {
    if buf.remaining_mut() < Cp56Time2a::ENCODED_SIZE {
        return Err(Error::BufferTooShort {
            need: Cp56Time2a::ENCODED_SIZE,
            have: buf.remaining_mut(),
        });
    }
    buf.put_slice(time.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::{BytesMut, Bytes};
    use crate::types::QualityDescriptor;

    #[test]
    fn type_id_roundtrip_single_point() {
        let spi = SinglePointInformation::new(true, QualityDescriptor::empty());
        let obj = InformationObject::SinglePoint(spi);
        assert_eq!(obj.type_id(), TypeId::MSpNa1);

        let mut buf = BytesMut::with_capacity(16);
        obj.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = InformationObject::decode(TypeId::MSpNa1, &mut reader).unwrap();
        assert_eq!(decoded.type_id(), TypeId::MSpNa1);
    }

    #[test]
    fn type_id_roundtrip_single_point_cp56() {
        let spi = SinglePointInformation::new(false, QualityDescriptor::INVALID);
        let mut time = Cp56Time2a::new();
        time.set_year(25);
        time.set_month(6);

        let obj = InformationObject::SinglePointCp56(spi, time);
        assert_eq!(obj.type_id(), TypeId::MSpTb1);
        assert_eq!(obj.encoded_size(), 1 + 7);

        let mut buf = BytesMut::with_capacity(16);
        obj.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = InformationObject::decode(TypeId::MSpTb1, &mut reader).unwrap();
        assert_eq!(obj, decoded);
    }

    #[test]
    fn type_id_roundtrip_measured_short_float_cp56() {
        let mv = MeasuredValueShortFloat::new(42.5, QualityDescriptor::empty());
        let time = Cp56Time2a::new();
        let obj = InformationObject::MeasuredShortFloatCp56(mv, time);
        assert_eq!(obj.type_id(), TypeId::MMeTf1);

        let mut buf = BytesMut::with_capacity(32);
        obj.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = InformationObject::decode(TypeId::MMeTf1, &mut reader).unwrap();
        assert_eq!(obj, decoded);
    }

    #[test]
    fn type_id_roundtrip_interrogation() {
        let obj = InformationObject::Interrogation(InterrogationCommand::station());
        assert_eq!(obj.type_id(), TypeId::CIcNa1);

        let mut buf = BytesMut::with_capacity(16);
        obj.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = InformationObject::decode(TypeId::CIcNa1, &mut reader).unwrap();
        assert_eq!(obj, decoded);
    }

    #[test]
    fn type_id_roundtrip_read_command() {
        let obj = InformationObject::Read(ReadCommand);
        assert_eq!(obj.type_id(), TypeId::CRdNa1);
        assert_eq!(obj.encoded_size(), 0);

        let mut buf = BytesMut::with_capacity(16);
        obj.encode(&mut buf).unwrap();
        let mut reader = Bytes::from(buf);
        let decoded = InformationObject::decode(TypeId::CRdNa1, &mut reader).unwrap();
        assert_eq!(obj, decoded);
    }

    #[test]
    fn unsupported_type_id() {
        let mut buf = Bytes::from_static(&[0x00]);
        assert!(InformationObject::decode(TypeId::MStNa1, &mut buf).is_err());
    }
}
