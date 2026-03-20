use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("encode error: {0}")]
    Encode(String),

    #[error("decode error: {0}")]
    Decode(String),

    #[error("connection error: {0}")]
    Connection(String),

    #[error("not connected")]
    NotConnected,

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("buffer too short: need {need} bytes, have {have}")]
    BufferTooShort { need: usize, have: usize },

    #[error("invalid value {value} for {type_name}")]
    InvalidValue { type_name: &'static str, value: u8 },

    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
