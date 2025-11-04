use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    /// More than one device in preloader mode is connected
    #[error("Please disconnect other devices in the preloader mode")]
    MoreThanOneDevice,

    /// The device returned invalid data when echoing bytes back
    #[error("Data doesn't match! Expected {0:#x}, got {1:#x}")]
    InvalidEchoData(u32, u32),
    /// The device returned invalid status of command
    #[error("Invalid status! Expected {0}, got {1}")]
    InvalidStatus(u16, u16),

    /// da-patcher error
    #[error("da-patcher error: {0}")]
    DAPatcher(#[from] da_patcher::err::Error),

    /// da-protocol error
    #[error("Protocol error: {0}")]
    DAProtocol(#[from] da_protocol::err::Error),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// serialport crate error
    #[error("serialport error: {0}")]
    SerialPort(#[from] serialport::Error),
    /// bincode crate error
    #[error("Bincode encode error: {0}")]
    BincodeEncode(#[from] bincode::error::EncodeError),
    /// Any other error
    #[error("{0}")]
    Custom(#[from] Box<dyn std::error::Error>),
}
