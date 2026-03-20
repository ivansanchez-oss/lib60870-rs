use std::fmt;

/// Originator address — identifies the source of an ASDU.
///
/// Present in the COT field when `size_of_cot == 2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OriginatorAddress(u8);

impl OriginatorAddress {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

impl From<u8> for OriginatorAddress {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl fmt::Display for OriginatorAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(OriginatorAddress::default().value(), 0);
    }

    #[test]
    fn from_u8() {
        let oa: OriginatorAddress = 42u8.into();
        assert_eq!(oa.value(), 42);
    }
}
