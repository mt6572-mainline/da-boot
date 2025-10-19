use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    #[error("Invalid magic")]
    InvalidMagic,
    #[error("Invalid struct data")]
    InvalidHeuristics,
    #[error("Invalid DA region count")]
    InvalidRegionCount,
    #[error("Invalid DA code start")]
    InvalidRegionStart,
    #[error("Invalid DA code size")]
    InvalidCodeSize,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Bincode decode error: {0}")]
    Bincode(#[from] bincode::error::DecodeError),
    #[error("{0}")]
    Custom(#[from] Box<dyn std::error::Error>),
}
