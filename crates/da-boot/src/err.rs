use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    /// The device returned invalid data when echoing bytes back
    #[error("Data doesn't match! Expected {0:#x}, got {1:#x}")]
    InvalidEchoData(u32, u32),
    /// The device returned invalid status of command
    #[error("Invalid status! Expected {0}, got {1}")]
    InvalidStatus(u16, u16),

    /// da-protocol error
    #[error("Protocol error: {0}")]
    DAProtocol(#[from] da_protocol::err::Error<std::io::Error>),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// I/O error
    #[error("Failed to convert from slice: {0}")]
    TryFromSlice(#[from] std::array::TryFromSliceError),
    /// serialport crate error
    #[error("serialport error: {0}")]
    SerialPort(#[from] serialport::Error),

    /// rustyline crate error
    #[error("rustyline crate error: {0}")]
    Rustyline(#[from] rustyline::error::ReadlineError),
}
