use thiserror::Error as TError;

/// Protocol errors
#[derive(Debug, TError)]
pub enum Error {
    /// postcard crate error
    #[error("postcard error: {0}")]
    Postcard(#[from] postcard::Error),
    /// `simpleport` error
    #[error("simpleport error: {0}")]
    Simpleport(#[from] simpleport::err::Error),
}
