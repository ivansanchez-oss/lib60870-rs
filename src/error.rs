use std::io;
use std::time::Duration;

// --- Frame layer (APCI) ---

#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    #[error("expected start byte 0x68, got 0x{0:02X}")]
    InvalidStartByte(u8),
    #[error("APDU length too short: {0}")]
    LengthTooShort(usize),
    #[error("APDU length {length} exceeds maximum {max}")]
    LengthExceeded { length: usize, max: usize },
    #[error("unknown U-frame control byte 0x{0:02X}")]
    UnknownUFunction(u8),
    #[error("invalid control field byte 0x{0:02X}")]
    InvalidControlField(u8),
    #[error(transparent)]
    Io(#[from] io::Error),
}

// --- ADU layer (ASDU encode/decode) ---

#[derive(Debug, thiserror::Error)]
pub enum AduError {
    #[error("buffer too short: need {need} bytes, have {have}")]
    BufferTooShort { need: usize, have: usize },
    #[error("invalid type id: {0}")]
    InvalidTypeId(u8),
    #[error("invalid cause of transmission: {0}")]
    InvalidCauseOfTransmission(u8),
    #[error("unsupported type id: {0}")]
    UnsupportedTypeId(u8),
    #[error("num_objects {0} exceeds maximum 127")]
    NumObjectsOverflow(u8),
    #[error("type mismatch: expected type id {expected}, got {got}")]
    TypeMismatch { expected: u8, got: u8 },
    #[error("sequential IOA at index {index}: expected {expected}, got {got}")]
    NonSequentialIoa { index: usize, expected: u32, got: u32 },
    #[error("ASDU size {size} exceeds max_asdu_length {max}")]
    ExceedsMaxLength { size: usize, max: u16 },
    #[error("ASDU must contain at least one object")]
    EmptyAsdu,
    #[error("ASDU cannot contain more than 127 objects")]
    TooManyObjects,
    #[error(transparent)]
    IoaOverflow(#[from] IoaOverflow),
}

/// IOA value exceeds the 3-byte maximum (0xFFFFFF).
#[derive(Debug, thiserror::Error)]
#[error("IOA value {0:#X} exceeds maximum 0xFFFFFF")]
pub struct IoaOverflow(pub u32);

// --- Configuration errors ---

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("k must be in 1..=32767, got {0}")]
    InvalidK(u16),
    #[error("w must be in 1..=32767, got {0}")]
    InvalidW(u16),
    #[error("w ({w}) must not exceed k ({k})")]
    WExceedsK { w: u16, k: u16 },
    #[error("{0} must be non-zero")]
    ZeroTimeout(&'static str),
    #[error("t2 ({t2:?}) must be less than t1 ({t1:?})")]
    T2NotLessThanT1 { t2: Duration, t1: Duration },
    #[error("size_of_cot must be 1 or 2, got {0}")]
    InvalidSizeOfCot(u8),
    #[error("size_of_ca must be 1 or 2, got {0}")]
    InvalidSizeOfCa(u8),
    #[error("size_of_ioa must be 1, 2, or 3, got {0}")]
    InvalidSizeOfIoa(u8),
    #[error("max_asdu_length ({max}) must be >= header size ({min})")]
    MaxAsduLengthTooSmall { max: u16, min: u16 },
}

// --- Client request errors ---

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("not connected")]
    NotConnected,
    #[error("client task shut down")]
    Shutdown,
    #[error("client task closed")]
    TaskClosed,
    #[error("send window full")]
    SendWindowFull,
    #[error("timeout: {0}")]
    Timeout(&'static str),
    #[error("sequence error: expected {expected}, got {got}")]
    SequenceError { expected: u16, got: u16 },
    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),
    #[error("data transfer not active")]
    NotActive,
    #[error("data transfer already active")]
    AlreadyActive,
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Adu(#[from] AduError),
    #[error(transparent)]
    Io(#[from] io::Error),
}
