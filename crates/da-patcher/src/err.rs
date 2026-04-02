use thiserror::Error as TError;

#[derive(Debug, TError)]
pub enum Error {
    /// Required pattern is not found
    #[error("Pattern not found")]
    PatternNotFound,
    /// Instruction mnemonic is not available due to capstone configuration
    #[error("Instruction mnemonic is not available")]
    MnemonicNotAvailable,
    /// Instruction value is not available due to capstone configuration
    #[error("Instruction as string is not available")]
    InstrOpNotAvailable,

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// `da-analyzer` crate error
    #[error("da-analyzer error: {0}")]
    Analyzer(#[from] da_analyzer::err::Error),

    /// `hexpatch_keystone` crate error
    #[error("Keystone error: {0}")]
    Keystone(hexpatch_keystone::Error),
    /// Parse int error
    #[error("Parse int error: {0}")]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("{0}")]
    /// Any other error
    Custom(#[from] Box<dyn std::error::Error>),
}

impl From<hexpatch_keystone::Error> for Error {
    fn from(value: hexpatch_keystone::Error) -> Self {
        Self::Keystone(value)
    }
}
