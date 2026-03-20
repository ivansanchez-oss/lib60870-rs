use crate::error::ConfigError;
use crate::types::OriginatorAddress;

/// Application layer parameters for ASDU encoding/decoding.
///
/// Controls the size of variable-length fields in the ASDU header.
/// Use [`AppLayerParametersBuilder`] for custom configurations with validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppLayerParameters {
    size_of_cot: u8,
    size_of_ca: u8,
    size_of_ioa: u8,
    max_asdu_length: u16,
    originator_address: OriginatorAddress,
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
        originator_address: OriginatorAddress::new(0),
    };

    /// Default parameters for IEC 60870-5-101.
    pub const CS101_DEFAULT: Self = Self {
        size_of_cot: 2,
        size_of_ca: 2,
        size_of_ioa: 3,
        max_asdu_length: 254,
        originator_address: OriginatorAddress::new(0),
    };

    /// Create a builder initialized with CS104 default values.
    pub fn builder() -> AppLayerParametersBuilder {
        AppLayerParametersBuilder::default()
    }

    /// Size of Cause of Transmission field (1 or 2 bytes).
    pub fn size_of_cot(&self) -> u8 {
        self.size_of_cot
    }

    /// Size of Common Address field (1 or 2 bytes).
    pub fn size_of_ca(&self) -> u8 {
        self.size_of_ca
    }

    /// Size of Information Object Address field (1, 2, or 3 bytes).
    pub fn size_of_ioa(&self) -> u8 {
        self.size_of_ioa
    }

    /// Maximum ASDU length in bytes.
    pub fn max_asdu_length(&self) -> u16 {
        self.max_asdu_length
    }

    /// Default originator address.
    pub fn originator_address(&self) -> OriginatorAddress {
        self.originator_address
    }

    /// Total ASDU header size: type_id(1) + vsq(1) + cot + ca.
    pub fn asdu_header_size(&self) -> usize {
        2 + self.size_of_cot as usize + self.size_of_ca as usize
    }
}

/// Builder for [`AppLayerParameters`] with standard constraint validation.
#[derive(Debug, Clone)]
pub struct AppLayerParametersBuilder {
    size_of_cot: u8,
    size_of_ca: u8,
    size_of_ioa: u8,
    max_asdu_length: u16,
    originator_address: OriginatorAddress,
}

impl Default for AppLayerParametersBuilder {
    fn default() -> Self {
        let defaults = AppLayerParameters::CS104_DEFAULT;
        Self {
            size_of_cot: defaults.size_of_cot,
            size_of_ca: defaults.size_of_ca,
            size_of_ioa: defaults.size_of_ioa,
            max_asdu_length: defaults.max_asdu_length,
            originator_address: defaults.originator_address,
        }
    }
}

impl AppLayerParametersBuilder {
    /// Size of Cause of Transmission field (1 or 2).
    pub fn size_of_cot(mut self, size: u8) -> Self {
        self.size_of_cot = size;
        self
    }

    /// Size of Common Address field (1 or 2).
    pub fn size_of_ca(mut self, size: u8) -> Self {
        self.size_of_ca = size;
        self
    }

    /// Size of Information Object Address field (1, 2, or 3).
    pub fn size_of_ioa(mut self, size: u8) -> Self {
        self.size_of_ioa = size;
        self
    }

    /// Maximum ASDU length in bytes.
    pub fn max_asdu_length(mut self, length: u16) -> Self {
        self.max_asdu_length = length;
        self
    }

    /// Default originator address.
    pub fn originator_address(mut self, addr: OriginatorAddress) -> Self {
        self.originator_address = addr;
        self
    }

    /// Build and validate the parameters.
    ///
    /// # Constraints
    /// - `size_of_cot`: 1 or 2
    /// - `size_of_ca`: 1 or 2
    /// - `size_of_ioa`: 1, 2, or 3
    /// - `max_asdu_length`: must be > header size
    pub fn build(self) -> Result<AppLayerParameters, ConfigError> {
        if self.size_of_cot < 1 || self.size_of_cot > 2 {
            return Err(ConfigError::InvalidSizeOfCot(self.size_of_cot));
        }
        if self.size_of_ca < 1 || self.size_of_ca > 2 {
            return Err(ConfigError::InvalidSizeOfCa(self.size_of_ca));
        }
        if self.size_of_ioa < 1 || self.size_of_ioa > 3 {
            return Err(ConfigError::InvalidSizeOfIoa(self.size_of_ioa));
        }
        let min_length = 2 + self.size_of_cot as u16 + self.size_of_ca as u16;
        if self.max_asdu_length < min_length {
            return Err(ConfigError::MaxAsduLengthTooSmall {
                max: self.max_asdu_length,
                min: min_length,
            });
        }

        Ok(AppLayerParameters {
            size_of_cot: self.size_of_cot,
            size_of_ca: self.size_of_ca,
            size_of_ioa: self.size_of_ioa,
            max_asdu_length: self.max_asdu_length,
            originator_address: self.originator_address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_cs104() {
        let params = AppLayerParameters::builder().build().unwrap();
        assert_eq!(params, AppLayerParameters::CS104_DEFAULT);
    }

    #[test]
    fn builder_custom() {
        let params = AppLayerParameters::builder()
            .size_of_cot(1)
            .size_of_ca(1)
            .size_of_ioa(2)
            .max_asdu_length(254)
            .build()
            .unwrap();
        assert_eq!(params.size_of_cot(), 1);
        assert_eq!(params.size_of_ca(), 1);
        assert_eq!(params.size_of_ioa(), 2);
    }

    #[test]
    fn rejects_invalid_cot_size() {
        assert!(AppLayerParameters::builder().size_of_cot(0).build().is_err());
        assert!(AppLayerParameters::builder().size_of_cot(3).build().is_err());
    }

    #[test]
    fn rejects_invalid_ca_size() {
        assert!(AppLayerParameters::builder().size_of_ca(0).build().is_err());
        assert!(AppLayerParameters::builder().size_of_ca(3).build().is_err());
    }

    #[test]
    fn rejects_invalid_ioa_size() {
        assert!(AppLayerParameters::builder().size_of_ioa(0).build().is_err());
        assert!(AppLayerParameters::builder().size_of_ioa(4).build().is_err());
    }

    #[test]
    fn rejects_max_asdu_too_small() {
        assert!(AppLayerParameters::builder()
            .max_asdu_length(3)
            .build()
            .is_err());
    }

    #[test]
    fn header_size() {
        assert_eq!(AppLayerParameters::CS104_DEFAULT.asdu_header_size(), 6);
        let params = AppLayerParameters::builder()
            .size_of_cot(1)
            .size_of_ca(1)
            .build()
            .unwrap();
        assert_eq!(params.asdu_header_size(), 4);
    }
}
