#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("BUG: Mapping raw offset to index failed, disassembler error")]
    MapOffsetToIndex,
    #[error("BUG: Basic block position can't be determined properly")]
    InvalidBlockIndex,
    #[error("BUG: Basic block analysis reached the next function due to split failure")]
    Overrun,
    #[error("BUG: PC overflowed the isize range")]
    PCOverflow,
}
