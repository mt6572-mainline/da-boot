#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No such string in the binary")]
    StringNotFound,
    #[error("String reference wasn't found by using both direct and literal pool scan")]
    StringReferenceNotFound,
    #[error("BUG: Mapping raw offset to index failed, disassembler error")]
    MapOffsetToIndex,
    #[error("BUG: Basic block position can't be determined properly")]
    InvalidBlockIndex,
    #[error("BUG: Basic block analysis reached the next function due to split failure")]
    Overrun,
    #[error("BUG: PC overflowed the isize range")]
    PCOverflow,
}
