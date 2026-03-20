/// Error returned when a raw u8 doesn't map to a known TypeId.
#[derive(Debug, thiserror::Error)]
#[error("invalid type id: {0}")]
pub struct InvalidTypeId(pub u8);

/// IEC 60870-5 type identification field.
///
/// Defines the type of information objects contained in an ASDU.
/// Values follow the IEC 60870-5-101/104 standard.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeId {
    // Process information in monitoring direction
    MSpNa1 = 1,   // Single-point information
    MSpTa1 = 2,   // Single-point with time tag
    MDpNa1 = 3,   // Double-point information
    MDpTa1 = 4,   // Double-point with time tag
    MStNa1 = 5,   // Step position information
    MStTa1 = 6,   // Step position with time tag
    MBoNa1 = 7,   // Bitstring of 32 bits
    MBoTa1 = 8,   // Bitstring with time tag
    MMeNa1 = 9,   // Measured value, normalized
    MMeTa1 = 10,  // Measured value, normalized with time tag
    MMeNb1 = 11,  // Measured value, scaled
    MMeTb1 = 12,  // Measured value, scaled with time tag
    MMeNc1 = 13,  // Measured value, short floating point
    MMeTc1 = 14,  // Measured value, short float with time tag
    MItNa1 = 15,  // Integrated totals
    MItTa1 = 16,  // Integrated totals with time tag
    MEpTa1 = 17,  // Event of protection equipment with time tag
    MEpTb1 = 18,  // Packed start events with time tag
    MEpTc1 = 19,  // Packed output circuit info with time tag
    MPsNa1 = 20,  // Packed single-point with status change detection
    MMeNd1 = 21,  // Measured value, normalized without quality descriptor

    // Process information in monitoring direction with CP56Time2a
    MSpTb1 = 30,  // Single-point with CP56Time2a
    MDpTb1 = 31,  // Double-point with CP56Time2a
    MStTb1 = 32,  // Step position with CP56Time2a
    MBoTb1 = 33,  // Bitstring with CP56Time2a
    MMeTd1 = 34,  // Measured value, normalized with CP56Time2a
    MMeTe1 = 35,  // Measured value, scaled with CP56Time2a
    MMeTf1 = 36,  // Measured value, short float with CP56Time2a
    MItTb1 = 37,  // Integrated totals with CP56Time2a
    MEpTd1 = 38,  // Event of protection with CP56Time2a
    MEpTe1 = 39,  // Packed start events with CP56Time2a
    MEpTf1 = 40,  // Packed output circuit info with CP56Time2a
    SItTc1 = 41,  // Integrated totals containing time-tagged security statistics

    // Process information in control direction
    CScNa1 = 45,  // Single command
    CDcNa1 = 46,  // Double command
    CRcNa1 = 47,  // Regulating step command
    CSeNa1 = 48,  // Set-point command, normalized value
    CSeNb1 = 49,  // Set-point command, scaled value
    CSeNc1 = 50,  // Set-point command, short floating point
    CBoNa1 = 51,  // Bitstring of 32 bits command

    // Process information in control direction with CP56Time2a
    CScTa1 = 58,  // Single command with CP56Time2a
    CDcTa1 = 59,  // Double command with CP56Time2a
    CRcTa1 = 60,  // Regulating step command with CP56Time2a
    CSeTa1 = 61,  // Set-point, normalized with CP56Time2a
    CSeTb1 = 62,  // Set-point, scaled with CP56Time2a
    CSeTc1 = 63,  // Set-point, short float with CP56Time2a
    CBoTa1 = 64,  // Bitstring command with CP56Time2a

    // System information in monitoring direction
    MEiNa1 = 70,  // End of initialization

    // Authentication
    SChNa1 = 81,  // Authentication challenge
    SRpNa1 = 82,  // Authentication reply
    SArNa1 = 83,  // Aggressive mode authentication request
    SKrNa1 = 84,  // Session key status request
    SKsNa1 = 85,  // Session key status
    SKcNa1 = 86,  // Session key change
    SErNa1 = 87,  // Authentication error

    // User management
    SUsNa1 = 90,  // User status change
    SUqNa1 = 91,  // Update key change request
    SUrNa1 = 92,  // Update key change reply
    SUkNa1 = 93,  // Update key change (symmetric)
    SUaNa1 = 94,  // Update key change (asymmetric)
    SUcNa1 = 95,  // Update key change confirmation

    // System information in control direction
    CIcNa1 = 100, // Interrogation command
    CCiNa1 = 101, // Counter interrogation command
    CRdNa1 = 102, // Read command
    CCsNa1 = 103, // Clock synchronization command
    CTsNa1 = 104, // Test command
    CRpNa1 = 105, // Reset process command
    CCdNa1 = 106, // Delay acquisition command
    CTsTa1 = 107, // Test command with CP56Time2a

    // Parameter in control direction
    PMeNa1 = 110, // Parameter of measured value, normalized
    PMeNb1 = 111, // Parameter of measured value, scaled
    PMeNc1 = 112, // Parameter of measured value, short float
    PAcNa1 = 113, // Parameter activation

    // File transfer
    FFrNa1 = 120, // File ready
    FSrNa1 = 121, // Section ready
    FScNa1 = 122, // Call directory, select file, call file, call section
    FLsNa1 = 123, // Last section, last segment
    FAfNa1 = 124, // ACK file, ACK section
    FSgNa1 = 125, // Segment
    FDrTa1 = 126, // Directory
    FScNb1 = 127, // QueryLog (request archive file)
}

impl TypeId {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for TypeId {
    type Error = InvalidTypeId;

    fn try_from(value: u8) -> Result<Self, InvalidTypeId> {
        match value {
            1 => Ok(Self::MSpNa1),
            2 => Ok(Self::MSpTa1),
            3 => Ok(Self::MDpNa1),
            4 => Ok(Self::MDpTa1),
            5 => Ok(Self::MStNa1),
            6 => Ok(Self::MStTa1),
            7 => Ok(Self::MBoNa1),
            8 => Ok(Self::MBoTa1),
            9 => Ok(Self::MMeNa1),
            10 => Ok(Self::MMeTa1),
            11 => Ok(Self::MMeNb1),
            12 => Ok(Self::MMeTb1),
            13 => Ok(Self::MMeNc1),
            14 => Ok(Self::MMeTc1),
            15 => Ok(Self::MItNa1),
            16 => Ok(Self::MItTa1),
            17 => Ok(Self::MEpTa1),
            18 => Ok(Self::MEpTb1),
            19 => Ok(Self::MEpTc1),
            20 => Ok(Self::MPsNa1),
            21 => Ok(Self::MMeNd1),
            30 => Ok(Self::MSpTb1),
            31 => Ok(Self::MDpTb1),
            32 => Ok(Self::MStTb1),
            33 => Ok(Self::MBoTb1),
            34 => Ok(Self::MMeTd1),
            35 => Ok(Self::MMeTe1),
            36 => Ok(Self::MMeTf1),
            37 => Ok(Self::MItTb1),
            38 => Ok(Self::MEpTd1),
            39 => Ok(Self::MEpTe1),
            40 => Ok(Self::MEpTf1),
            41 => Ok(Self::SItTc1),
            45 => Ok(Self::CScNa1),
            46 => Ok(Self::CDcNa1),
            47 => Ok(Self::CRcNa1),
            48 => Ok(Self::CSeNa1),
            49 => Ok(Self::CSeNb1),
            50 => Ok(Self::CSeNc1),
            51 => Ok(Self::CBoNa1),
            58 => Ok(Self::CScTa1),
            59 => Ok(Self::CDcTa1),
            60 => Ok(Self::CRcTa1),
            61 => Ok(Self::CSeTa1),
            62 => Ok(Self::CSeTb1),
            63 => Ok(Self::CSeTc1),
            64 => Ok(Self::CBoTa1),
            70 => Ok(Self::MEiNa1),
            81 => Ok(Self::SChNa1),
            82 => Ok(Self::SRpNa1),
            83 => Ok(Self::SArNa1),
            84 => Ok(Self::SKrNa1),
            85 => Ok(Self::SKsNa1),
            86 => Ok(Self::SKcNa1),
            87 => Ok(Self::SErNa1),
            90 => Ok(Self::SUsNa1),
            91 => Ok(Self::SUqNa1),
            92 => Ok(Self::SUrNa1),
            93 => Ok(Self::SUkNa1),
            94 => Ok(Self::SUaNa1),
            95 => Ok(Self::SUcNa1),
            100 => Ok(Self::CIcNa1),
            101 => Ok(Self::CCiNa1),
            102 => Ok(Self::CRdNa1),
            103 => Ok(Self::CCsNa1),
            104 => Ok(Self::CTsNa1),
            105 => Ok(Self::CRpNa1),
            106 => Ok(Self::CCdNa1),
            107 => Ok(Self::CTsTa1),
            110 => Ok(Self::PMeNa1),
            111 => Ok(Self::PMeNb1),
            112 => Ok(Self::PMeNc1),
            113 => Ok(Self::PAcNa1),
            120 => Ok(Self::FFrNa1),
            121 => Ok(Self::FSrNa1),
            122 => Ok(Self::FScNa1),
            123 => Ok(Self::FLsNa1),
            124 => Ok(Self::FAfNa1),
            125 => Ok(Self::FSgNa1),
            126 => Ok(Self::FDrTa1),
            127 => Ok(Self::FScNb1),
            _ => Err(InvalidTypeId(value)),
        }
    }
}

impl std::fmt::Display for TypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::MSpNa1 => "M_SP_NA_1",
            Self::MSpTa1 => "M_SP_TA_1",
            Self::MDpNa1 => "M_DP_NA_1",
            Self::MDpTa1 => "M_DP_TA_1",
            Self::MStNa1 => "M_ST_NA_1",
            Self::MStTa1 => "M_ST_TA_1",
            Self::MBoNa1 => "M_BO_NA_1",
            Self::MBoTa1 => "M_BO_TA_1",
            Self::MMeNa1 => "M_ME_NA_1",
            Self::MMeTa1 => "M_ME_TA_1",
            Self::MMeNb1 => "M_ME_NB_1",
            Self::MMeTb1 => "M_ME_TB_1",
            Self::MMeNc1 => "M_ME_NC_1",
            Self::MMeTc1 => "M_ME_TC_1",
            Self::MItNa1 => "M_IT_NA_1",
            Self::MItTa1 => "M_IT_TA_1",
            Self::MEpTa1 => "M_EP_TA_1",
            Self::MEpTb1 => "M_EP_TB_1",
            Self::MEpTc1 => "M_EP_TC_1",
            Self::MPsNa1 => "M_PS_NA_1",
            Self::MMeNd1 => "M_ME_ND_1",
            Self::MSpTb1 => "M_SP_TB_1",
            Self::MDpTb1 => "M_DP_TB_1",
            Self::MStTb1 => "M_ST_TB_1",
            Self::MBoTb1 => "M_BO_TB_1",
            Self::MMeTd1 => "M_ME_TD_1",
            Self::MMeTe1 => "M_ME_TE_1",
            Self::MMeTf1 => "M_ME_TF_1",
            Self::MItTb1 => "M_IT_TB_1",
            Self::MEpTd1 => "M_EP_TD_1",
            Self::MEpTe1 => "M_EP_TE_1",
            Self::MEpTf1 => "M_EP_TF_1",
            Self::SItTc1 => "S_IT_TC_1",
            Self::CScNa1 => "C_SC_NA_1",
            Self::CDcNa1 => "C_DC_NA_1",
            Self::CRcNa1 => "C_RC_NA_1",
            Self::CSeNa1 => "C_SE_NA_1",
            Self::CSeNb1 => "C_SE_NB_1",
            Self::CSeNc1 => "C_SE_NC_1",
            Self::CBoNa1 => "C_BO_NA_1",
            Self::CScTa1 => "C_SC_TA_1",
            Self::CDcTa1 => "C_DC_TA_1",
            Self::CRcTa1 => "C_RC_TA_1",
            Self::CSeTa1 => "C_SE_TA_1",
            Self::CSeTb1 => "C_SE_TB_1",
            Self::CSeTc1 => "C_SE_TC_1",
            Self::CBoTa1 => "C_BO_TA_1",
            Self::MEiNa1 => "M_EI_NA_1",
            Self::SChNa1 => "S_CH_NA_1",
            Self::SRpNa1 => "S_RP_NA_1",
            Self::SArNa1 => "S_AR_NA_1",
            Self::SKrNa1 => "S_KR_NA_1",
            Self::SKsNa1 => "S_KS_NA_1",
            Self::SKcNa1 => "S_KC_NA_1",
            Self::SErNa1 => "S_ER_NA_1",
            Self::SUsNa1 => "S_US_NA_1",
            Self::SUqNa1 => "S_UQ_NA_1",
            Self::SUrNa1 => "S_UR_NA_1",
            Self::SUkNa1 => "S_UK_NA_1",
            Self::SUaNa1 => "S_UA_NA_1",
            Self::SUcNa1 => "S_UC_NA_1",
            Self::CIcNa1 => "C_IC_NA_1",
            Self::CCiNa1 => "C_CI_NA_1",
            Self::CRdNa1 => "C_RD_NA_1",
            Self::CCsNa1 => "C_CS_NA_1",
            Self::CTsNa1 => "C_TS_NA_1",
            Self::CRpNa1 => "C_RP_NA_1",
            Self::CCdNa1 => "C_CD_NA_1",
            Self::CTsTa1 => "C_TS_TA_1",
            Self::PMeNa1 => "P_ME_NA_1",
            Self::PMeNb1 => "P_ME_NB_1",
            Self::PMeNc1 => "P_ME_NC_1",
            Self::PAcNa1 => "P_AC_NA_1",
            Self::FFrNa1 => "F_FR_NA_1",
            Self::FSrNa1 => "F_SR_NA_1",
            Self::FScNa1 => "F_SC_NA_1",
            Self::FLsNa1 => "F_LS_NA_1",
            Self::FAfNa1 => "F_AF_NA_1",
            Self::FSgNa1 => "F_SG_NA_1",
            Self::FDrTa1 => "F_DR_TA_1",
            Self::FScNb1 => "F_SC_NB_1",
        };
        write!(f, "{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_all_type_ids() {
        let all_values: &[u8] = &[
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
            30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41,
            45, 46, 47, 48, 49, 50, 51,
            58, 59, 60, 61, 62, 63, 64,
            70,
            81, 82, 83, 84, 85, 86, 87,
            90, 91, 92, 93, 94, 95,
            100, 101, 102, 103, 104, 105, 106, 107,
            110, 111, 112, 113,
            120, 121, 122, 123, 124, 125, 126, 127,
        ];

        for &v in all_values {
            let tid = TypeId::try_from(v).unwrap();
            assert_eq!(tid.as_u8(), v);
        }
    }

    #[test]
    fn invalid_type_id() {
        assert!(TypeId::try_from(0).is_err());
        assert!(TypeId::try_from(22).is_err());
        assert!(TypeId::try_from(128).is_err());
        assert!(TypeId::try_from(255).is_err());
    }

    #[test]
    fn display_format() {
        assert_eq!(TypeId::MSpNa1.to_string(), "M_SP_NA_1");
        assert_eq!(TypeId::CIcNa1.to_string(), "C_IC_NA_1");
        assert_eq!(TypeId::MMeNc1.to_string(), "M_ME_NC_1");
    }
}
