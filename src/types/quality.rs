use bitflags::bitflags;

bitflags! {
    /// Quality descriptor for measured and status values (IEC 60870-5-101 7.2.6.3).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct QualityDescriptor: u8 {
        const OVERFLOW    = 0x01;
        const BLOCKED     = 0x10;
        const SUBSTITUTED = 0x20;
        const NON_TOPICAL = 0x40;
        const INVALID     = 0x80;
    }
}

bitflags! {
    /// Quality descriptor for protection equipment (IEC 60870-5-101 7.2.6.4).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct QualityDescriptorP: u8 {
        const RESERVED             = 0x04;
        const ELAPSED_TIME_INVALID = 0x08;
        const BLOCKED              = 0x10;
        const SUBSTITUTED          = 0x20;
        const NON_TOPICAL          = 0x40;
        const INVALID              = 0x80;
    }
}

bitflags! {
    /// Start events of protection equipment (IEC 60870-5-101 7.2.6.11).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct StartEvent: u8 {
        const GS  = 0x01; // General start
        const SL1 = 0x02; // Start of phase L1
        const SL2 = 0x04; // Start of phase L2
        const SL3 = 0x08; // Start of phase L3
        const SIE = 0x10; // Start of earth current
        const SRD = 0x20; // Start of reverse direction
    }
}

bitflags! {
    /// Output circuit information of protection equipment (IEC 60870-5-101 7.2.6.12).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct OutputCircuitInfo: u8 {
        const GC  = 0x01; // General command to output circuit
        const CL1 = 0x02; // Command to output circuit of phase L1
        const CL2 = 0x04; // Command to output circuit of phase L2
        const CL3 = 0x08; // Command to output circuit of phase L3
    }
}

/// Double point value states.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DoublePointValue {
    Intermediate = 0,
    Off = 1,
    On = 2,
    Indeterminate = 3,
}

impl DoublePointValue {
    pub fn from_raw(val: u8) -> Self {
        match val & 0x03 {
            0 => Self::Intermediate,
            1 => Self::Off,
            2 => Self::On,
            _ => Self::Indeterminate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_descriptor_flags() {
        let q = QualityDescriptor::OVERFLOW | QualityDescriptor::INVALID;
        assert!(q.contains(QualityDescriptor::OVERFLOW));
        assert!(q.contains(QualityDescriptor::INVALID));
        assert!(!q.contains(QualityDescriptor::BLOCKED));
        assert_eq!(q.bits(), 0x81);
    }

    #[test]
    fn quality_good_is_empty() {
        let q = QualityDescriptor::empty();
        assert_eq!(q.bits(), 0x00);
        assert!(q.is_empty());
    }

    #[test]
    fn double_point_from_raw() {
        assert_eq!(DoublePointValue::from_raw(0), DoublePointValue::Intermediate);
        assert_eq!(DoublePointValue::from_raw(1), DoublePointValue::Off);
        assert_eq!(DoublePointValue::from_raw(2), DoublePointValue::On);
        assert_eq!(DoublePointValue::from_raw(3), DoublePointValue::Indeterminate);
        // Upper bits are masked
        assert_eq!(DoublePointValue::from_raw(0xFE), DoublePointValue::On);
    }
}
