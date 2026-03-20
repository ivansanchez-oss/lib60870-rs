use std::fmt;

/// Common address of ASDU (CA) — identifies the station.
///
/// Encoded as 1 or 2 bytes on the wire depending on `size_of_ca`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommonAddress(u16);

impl CommonAddress {
    /// Broadcast / global address.
    pub const GLOBAL: Self = Self(0xFFFF);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }
}

impl From<u16> for CommonAddress {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<u8> for CommonAddress {
    fn from(value: u8) -> Self {
        Self(value as u16)
    }
}

impl fmt::Display for CommonAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_u16() {
        let ca = CommonAddress::new(0x1234);
        assert_eq!(ca.value(), 0x1234);
    }

    #[test]
    fn from_u8() {
        let ca: CommonAddress = 5u8.into();
        assert_eq!(ca.value(), 5);
    }

    #[test]
    fn global() {
        assert_eq!(CommonAddress::GLOBAL.value(), 0xFFFF);
    }
}
