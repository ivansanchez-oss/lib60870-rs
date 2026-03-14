use crate::error::Error;

/// Cause of transmission as defined in IEC 60870-5-101 section 7.2.3.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CauseOfTransmission {
    Periodic = 1,
    BackgroundScan = 2,
    Spontaneous = 3,
    Initialized = 4,
    Request = 5,
    Activation = 6,
    ActivationCon = 7,
    Deactivation = 8,
    DeactivationCon = 9,
    ActivationTermination = 10,
    ReturnInfoRemote = 11,
    ReturnInfoLocal = 12,
    FileTransfer = 13,
    Authentication = 14,
    MaintenanceOfAuthSessionKey = 15,
    MaintenanceOfUserRoleAndUpdateKey = 16,
    InterrogatedByStation = 20,
    InterrogatedByGroup1 = 21,
    InterrogatedByGroup2 = 22,
    InterrogatedByGroup3 = 23,
    InterrogatedByGroup4 = 24,
    InterrogatedByGroup5 = 25,
    InterrogatedByGroup6 = 26,
    InterrogatedByGroup7 = 27,
    InterrogatedByGroup8 = 28,
    InterrogatedByGroup9 = 29,
    InterrogatedByGroup10 = 30,
    InterrogatedByGroup11 = 31,
    InterrogatedByGroup12 = 32,
    InterrogatedByGroup13 = 33,
    InterrogatedByGroup14 = 34,
    InterrogatedByGroup15 = 35,
    InterrogatedByGroup16 = 36,
    RequestedByGeneralCounter = 37,
    RequestedByGroup1Counter = 38,
    RequestedByGroup2Counter = 39,
    RequestedByGroup3Counter = 40,
    RequestedByGroup4Counter = 41,
    UnknownTypeId = 44,
    UnknownCot = 45,
    UnknownCa = 46,
    UnknownIoa = 47,
}

impl CauseOfTransmission {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for CauseOfTransmission {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Error> {
        match value {
            1 => Ok(Self::Periodic),
            2 => Ok(Self::BackgroundScan),
            3 => Ok(Self::Spontaneous),
            4 => Ok(Self::Initialized),
            5 => Ok(Self::Request),
            6 => Ok(Self::Activation),
            7 => Ok(Self::ActivationCon),
            8 => Ok(Self::Deactivation),
            9 => Ok(Self::DeactivationCon),
            10 => Ok(Self::ActivationTermination),
            11 => Ok(Self::ReturnInfoRemote),
            12 => Ok(Self::ReturnInfoLocal),
            13 => Ok(Self::FileTransfer),
            14 => Ok(Self::Authentication),
            15 => Ok(Self::MaintenanceOfAuthSessionKey),
            16 => Ok(Self::MaintenanceOfUserRoleAndUpdateKey),
            20 => Ok(Self::InterrogatedByStation),
            21 => Ok(Self::InterrogatedByGroup1),
            22 => Ok(Self::InterrogatedByGroup2),
            23 => Ok(Self::InterrogatedByGroup3),
            24 => Ok(Self::InterrogatedByGroup4),
            25 => Ok(Self::InterrogatedByGroup5),
            26 => Ok(Self::InterrogatedByGroup6),
            27 => Ok(Self::InterrogatedByGroup7),
            28 => Ok(Self::InterrogatedByGroup8),
            29 => Ok(Self::InterrogatedByGroup9),
            30 => Ok(Self::InterrogatedByGroup10),
            31 => Ok(Self::InterrogatedByGroup11),
            32 => Ok(Self::InterrogatedByGroup12),
            33 => Ok(Self::InterrogatedByGroup13),
            34 => Ok(Self::InterrogatedByGroup14),
            35 => Ok(Self::InterrogatedByGroup15),
            36 => Ok(Self::InterrogatedByGroup16),
            37 => Ok(Self::RequestedByGeneralCounter),
            38 => Ok(Self::RequestedByGroup1Counter),
            39 => Ok(Self::RequestedByGroup2Counter),
            40 => Ok(Self::RequestedByGroup3Counter),
            41 => Ok(Self::RequestedByGroup4Counter),
            44 => Ok(Self::UnknownTypeId),
            45 => Ok(Self::UnknownCot),
            46 => Ok(Self::UnknownCa),
            47 => Ok(Self::UnknownIoa),
            _ => Err(Error::InvalidValue {
                type_name: "CauseOfTransmission",
                value,
            }),
        }
    }
}

impl std::fmt::Display for CauseOfTransmission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let cases = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
                     20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36,
                     37, 38, 39, 40, 41, 44, 45, 46, 47];
        for v in cases {
            let cot = CauseOfTransmission::try_from(v).unwrap();
            assert_eq!(cot.as_u8(), v);
        }
    }

    #[test]
    fn invalid_values() {
        assert!(CauseOfTransmission::try_from(0).is_err());
        assert!(CauseOfTransmission::try_from(17).is_err());
        assert!(CauseOfTransmission::try_from(42).is_err());
        assert!(CauseOfTransmission::try_from(48).is_err());
    }
}
