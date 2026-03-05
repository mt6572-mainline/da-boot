use std::ops::Not;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuMode {
    Arm,
    Thumb,
}

impl Not for CpuMode {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::Arm => Self::Thumb,
            Self::Thumb => Self::Arm,
        }
    }
}
