use ufmt::{uDisplay, uwrite};

pub enum ParamsError {
    InvalidMagic,
    InvalidVersion,
    InvalidMemoryRange,
    InvalidFnPtr,
    RunningInBlacklistedRange,
}

impl uDisplay for ParamsError {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        match self {
            Self::InvalidMagic => uwrite!(f, "invalid magic"),
            Self::InvalidVersion => uwrite!(f, "invalid version"),
            Self::InvalidMemoryRange => uwrite!(f, "invalid memory range"),
            Self::InvalidFnPtr => uwrite!(f, "invalid dl/ul fn ptr"),
            Self::RunningInBlacklistedRange => uwrite!(f, "running in the blacklisted memory range, this is a bad idea"),
        }
    }
}
