use thiserror::Error as TError;

/// Protocol errors
#[derive(Debug, TError)]
pub enum Error<E> {
    /// postcard crate error
    #[error("postcard error: {0}")]
    Postcard(#[from] postcard::Error),

    #[error("transport error: {0}")]
    Transport(E),
}
