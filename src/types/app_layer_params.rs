/// Application layer parameters for ASDU encoding/decoding.
///
/// Controls the size of variable-length fields in the ASDU header.
/// Default values follow IEC 60870-5-104.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayerParameters {
    pub size_of_cot: u8,
    pub size_of_ca: u8,
    pub size_of_ioa: u8,
    pub max_asdu_length: u16,
    pub originator_address: u8,
}

impl Default for AppLayerParameters {
    fn default() -> Self {
        Self::CS104_DEFAULT
    }
}

impl AppLayerParameters {
    /// Default parameters for IEC 60870-5-104.
    pub const CS104_DEFAULT: Self = Self {
        size_of_cot: 2,
        size_of_ca: 2,
        size_of_ioa: 3,
        max_asdu_length: 249,
        originator_address: 0,
    };

    /// Default parameters for IEC 60870-5-101.
    pub const CS101_DEFAULT: Self = Self {
        size_of_cot: 2,
        size_of_ca: 2,
        size_of_ioa: 3,
        max_asdu_length: 254,
        originator_address: 0,
    };

    /// Total ASDU header size: type_id(1) + vsq(1) + cot + ca.
    pub fn asdu_header_size(&self) -> usize {
        2 + self.size_of_cot as usize + self.size_of_ca as usize
    }
}
