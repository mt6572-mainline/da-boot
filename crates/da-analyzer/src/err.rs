#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Pattern not found")]
    NotFound,
}
