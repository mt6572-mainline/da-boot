pub enum SoC {
    MT6572,
}

impl SoC {
    /// Convert hwcode into SoC.
    pub fn try_from_hwcode(hwcode: u16) -> Option<Self> {
        match hwcode {
            0x6572 => Some(Self::MT6572),
            _ => None,
        }
    }

    /// SoC hwcode.
    pub fn as_hwcode(&self) -> u16 {
        match self {
            Self::MT6572 => 0x6572,
        }
    }

    /// DA1 SRAM address.
    pub fn da_sram_addr(&self) -> u32 {
        match self {
            Self::MT6572 => 0x2007000,
        }
    }

    /// DA1 DRAM address.
    pub fn da_dram_addr(&self) -> u32 {
        match self {
            Self::MT6572 => 0x81e00000,
        }
    }

    /// Preloader address.
    pub fn preloader_addr(&self) -> u32 {
        match self {
            Self::MT6572 => 0x2007500,
        }
    }
}
