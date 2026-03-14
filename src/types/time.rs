use crate::error::{Error, Result};

fn check_len(buf_len: usize, required: usize) -> Result<()> {
    if buf_len < required {
        return Err(Error::BufferTooShort {
            need: required,
            have: buf_len,
        });
    }
    Ok(())
}

// Reads the raw u16 from bytes [0..1] which encodes (seconds * 1000 + milliseconds).
// Used by CP24Time2a and CP56Time2a.
fn ms_raw(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

fn set_ms_raw(bytes: &mut [u8], value: u16) {
    let le = value.to_le_bytes();
    bytes[0] = le[0];
    bytes[1] = le[1];
}

/// CP16Time2a - Elapsed time, 2 bytes.
///
/// Wire format: `[ms_low, ms_high]` (little-endian milliseconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cp16Time2a {
    bytes: [u8; 2],
}

impl Cp16Time2a {
    pub const ENCODED_SIZE: usize = 2;

    pub fn new() -> Self {
        Self { bytes: [0; 2] }
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        check_len(buf.len(), Self::ENCODED_SIZE)?;
        let mut bytes = [0u8; 2];
        bytes.copy_from_slice(&buf[..2]);
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8; 2] {
        &self.bytes
    }

    pub fn elapsed_time_ms(&self) -> u16 {
        u16::from_le_bytes(self.bytes)
    }

    pub fn set_elapsed_time_ms(&mut self, value: u16) {
        self.bytes = value.to_le_bytes();
    }
}

impl Default for Cp16Time2a {
    fn default() -> Self {
        Self::new()
    }
}

/// CP24Time2a - Three-byte time (ms within minute + minute + quality flags).
///
/// Wire format:
///   `[0..1]` milliseconds within minute (seconds * 1000 + ms, little-endian)
///   `[2]`    bits 0-5: minute, bit 6: substituted, bit 7: invalid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cp24Time2a {
    bytes: [u8; 3],
}

impl Cp24Time2a {
    pub const ENCODED_SIZE: usize = 3;

    pub fn new() -> Self {
        Self { bytes: [0; 3] }
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        check_len(buf.len(), Self::ENCODED_SIZE)?;
        let mut bytes = [0u8; 3];
        bytes.copy_from_slice(&buf[..3]);
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8; 3] {
        &self.bytes
    }

    pub fn millisecond(&self) -> u16 {
        ms_raw(&self.bytes) % 1000
    }

    pub fn set_millisecond(&mut self, value: u16) {
        let seconds_part = ms_raw(&self.bytes) / 1000 * 1000;
        set_ms_raw(&mut self.bytes, seconds_part + value);
    }

    pub fn second(&self) -> u8 {
        (ms_raw(&self.bytes) / 1000) as u8
    }

    pub fn set_second(&mut self, value: u8) {
        let ms_part = ms_raw(&self.bytes) % 1000;
        set_ms_raw(&mut self.bytes, (value as u16) * 1000 + ms_part);
    }

    pub fn minute(&self) -> u8 {
        self.bytes[2] & 0x3f
    }

    pub fn set_minute(&mut self, value: u8) {
        self.bytes[2] = (self.bytes[2] & 0xc0) | (value & 0x3f);
    }

    pub fn is_invalid(&self) -> bool {
        self.bytes[2] & 0x80 != 0
    }

    pub fn set_invalid(&mut self, value: bool) {
        if value {
            self.bytes[2] |= 0x80;
        } else {
            self.bytes[2] &= 0x7f;
        }
    }

    pub fn is_substituted(&self) -> bool {
        self.bytes[2] & 0x40 != 0
    }

    pub fn set_substituted(&mut self, value: bool) {
        if value {
            self.bytes[2] |= 0x40;
        } else {
            self.bytes[2] &= 0xbf;
        }
    }
}

impl Default for Cp24Time2a {
    fn default() -> Self {
        Self::new()
    }
}

/// CP56Time2a - Seven-byte calendar time (IEC 60870-5-101 section 7.2.6.18).
///
/// Wire format:
///   `[0..1]` milliseconds within minute (seconds * 1000 + ms, little-endian)
///   `[2]`    bits 0-5: minute, bit 6: substituted, bit 7: invalid
///   `[3]`    bits 0-4: hour, bit 7: summer time
///   `[4]`    bits 0-4: day of month, bits 5-7: day of week
///   `[5]`    bits 0-3: month (1-12)
///   `[6]`    bits 0-6: year (0-99, relative to 2000)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cp56Time2a {
    bytes: [u8; 7],
}

impl Cp56Time2a {
    pub const ENCODED_SIZE: usize = 7;

    pub fn new() -> Self {
        Self { bytes: [0; 7] }
    }

    pub fn from_bytes(buf: &[u8]) -> Result<Self> {
        check_len(buf.len(), Self::ENCODED_SIZE)?;
        let mut bytes = [0u8; 7];
        bytes.copy_from_slice(&buf[..7]);
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8; 7] {
        &self.bytes
    }

    // Bytes [0..2]: same layout as CP24Time2a

    pub fn millisecond(&self) -> u16 {
        ms_raw(&self.bytes) % 1000
    }

    pub fn set_millisecond(&mut self, value: u16) {
        let seconds_part = ms_raw(&self.bytes) / 1000 * 1000;
        set_ms_raw(&mut self.bytes, seconds_part + value);
    }

    pub fn second(&self) -> u8 {
        (ms_raw(&self.bytes) / 1000) as u8
    }

    pub fn set_second(&mut self, value: u8) {
        let ms_part = ms_raw(&self.bytes) % 1000;
        set_ms_raw(&mut self.bytes, (value as u16) * 1000 + ms_part);
    }

    pub fn minute(&self) -> u8 {
        self.bytes[2] & 0x3f
    }

    pub fn set_minute(&mut self, value: u8) {
        self.bytes[2] = (self.bytes[2] & 0xc0) | (value & 0x3f);
    }

    pub fn is_invalid(&self) -> bool {
        self.bytes[2] & 0x80 != 0
    }

    pub fn set_invalid(&mut self, value: bool) {
        if value {
            self.bytes[2] |= 0x80;
        } else {
            self.bytes[2] &= 0x7f;
        }
    }

    pub fn is_substituted(&self) -> bool {
        self.bytes[2] & 0x40 != 0
    }

    pub fn set_substituted(&mut self, value: bool) {
        if value {
            self.bytes[2] |= 0x40;
        } else {
            self.bytes[2] &= 0xbf;
        }
    }

    // Byte [3]: hour + summer time

    pub fn hour(&self) -> u8 {
        self.bytes[3] & 0x1f
    }

    pub fn set_hour(&mut self, value: u8) {
        self.bytes[3] = (self.bytes[3] & 0xe0) | (value & 0x1f);
    }

    pub fn is_summer_time(&self) -> bool {
        self.bytes[3] & 0x80 != 0
    }

    pub fn set_summer_time(&mut self, value: bool) {
        if value {
            self.bytes[3] |= 0x80;
        } else {
            self.bytes[3] &= 0x7f;
        }
    }

    // Byte [4]: day of month + day of week

    pub fn day_of_month(&self) -> u8 {
        self.bytes[4] & 0x1f
    }

    pub fn set_day_of_month(&mut self, value: u8) {
        self.bytes[4] = (self.bytes[4] & 0xe0) | (value & 0x1f);
    }

    pub fn day_of_week(&self) -> u8 {
        (self.bytes[4] & 0xe0) >> 5
    }

    pub fn set_day_of_week(&mut self, value: u8) {
        self.bytes[4] = (self.bytes[4] & 0x1f) | ((value & 0x07) << 5);
    }

    // Byte [5]: month

    pub fn month(&self) -> u8 {
        self.bytes[5] & 0x0f
    }

    pub fn set_month(&mut self, value: u8) {
        self.bytes[5] = (self.bytes[5] & 0xf0) | (value & 0x0f);
    }

    // Byte [6]: year

    pub fn year(&self) -> u8 {
        self.bytes[6] & 0x7f
    }

    pub fn set_year(&mut self, value: u8) {
        self.bytes[6] = (self.bytes[6] & 0x80) | (value % 100 & 0x7f);
    }
}

impl Default for Cp56Time2a {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cp16_roundtrip() {
        let mut t = Cp16Time2a::new();
        t.set_elapsed_time_ms(12345);
        assert_eq!(t.elapsed_time_ms(), 12345);

        let decoded = Cp16Time2a::from_bytes(t.as_bytes()).unwrap();
        assert_eq!(decoded.elapsed_time_ms(), 12345);
    }

    #[test]
    fn cp24_getters_setters() {
        let mut t = Cp24Time2a::new();
        t.set_second(34);
        t.set_millisecond(567);
        t.set_minute(45);
        t.set_invalid(true);
        t.set_substituted(false);

        assert_eq!(t.second(), 34);
        assert_eq!(t.millisecond(), 567);
        assert_eq!(t.minute(), 45);
        assert!(t.is_invalid());
        assert!(!t.is_substituted());
    }

    #[test]
    fn cp24_roundtrip() {
        let mut t = Cp24Time2a::new();
        t.set_second(12);
        t.set_millisecond(345);
        t.set_minute(59);
        t.set_invalid(true);

        let decoded = Cp24Time2a::from_bytes(t.as_bytes()).unwrap();
        assert_eq!(decoded.second(), 12);
        assert_eq!(decoded.millisecond(), 345);
        assert_eq!(decoded.minute(), 59);
        assert!(decoded.is_invalid());
        assert!(!decoded.is_substituted());
    }

    #[test]
    fn cp56_getters_setters() {
        let mut t = Cp56Time2a::new();
        t.set_second(45);
        t.set_millisecond(123);
        t.set_minute(30);
        t.set_hour(14);
        t.set_day_of_month(15);
        t.set_day_of_week(3);
        t.set_month(6);
        t.set_year(25);
        t.set_summer_time(true);
        t.set_invalid(false);
        t.set_substituted(true);

        assert_eq!(t.second(), 45);
        assert_eq!(t.millisecond(), 123);
        assert_eq!(t.minute(), 30);
        assert_eq!(t.hour(), 14);
        assert_eq!(t.day_of_month(), 15);
        assert_eq!(t.day_of_week(), 3);
        assert_eq!(t.month(), 6);
        assert_eq!(t.year(), 25);
        assert!(t.is_summer_time());
        assert!(!t.is_invalid());
        assert!(t.is_substituted());
    }

    #[test]
    fn cp56_roundtrip() {
        let mut t = Cp56Time2a::new();
        t.set_second(45);
        t.set_millisecond(123);
        t.set_minute(30);
        t.set_hour(14);
        t.set_day_of_month(15);
        t.set_day_of_week(3);
        t.set_month(6);
        t.set_year(25);
        t.set_summer_time(true);
        t.set_substituted(true);

        let decoded = Cp56Time2a::from_bytes(t.as_bytes()).unwrap();
        assert_eq!(t, decoded);
    }

    #[test]
    fn cp56_all_flags() {
        let mut t = Cp56Time2a::new();
        t.set_summer_time(true);
        t.set_invalid(true);
        t.set_substituted(true);

        let bytes = t.as_bytes();
        assert!(bytes[2] & 0x80 != 0); // invalid
        assert!(bytes[2] & 0x40 != 0); // substituted
        assert!(bytes[3] & 0x80 != 0); // summer time

        let decoded = Cp56Time2a::from_bytes(bytes).unwrap();
        assert!(decoded.is_invalid());
        assert!(decoded.is_substituted());
        assert!(decoded.is_summer_time());
    }

    #[test]
    fn cp56_set_second_preserves_millisecond() {
        let mut t = Cp56Time2a::new();
        t.set_millisecond(456);
        t.set_second(30);
        assert_eq!(t.millisecond(), 456);
        assert_eq!(t.second(), 30);
    }

    #[test]
    fn cp56_set_millisecond_preserves_second() {
        let mut t = Cp56Time2a::new();
        t.set_second(30);
        t.set_millisecond(789);
        assert_eq!(t.second(), 30);
        assert_eq!(t.millisecond(), 789);
    }

    #[test]
    fn buffer_too_short() {
        assert!(Cp16Time2a::from_bytes(&[0]).is_err());
        assert!(Cp24Time2a::from_bytes(&[0, 0]).is_err());
        assert!(Cp56Time2a::from_bytes(&[0; 6]).is_err());
    }
}
