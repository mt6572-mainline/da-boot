#![no_std]
#![feature(const_trait_impl, const_default)]
use core::ops::Range;

use acon::SoC;

use crate::err::Error;

pub mod err;

pub const MAGIC: u32 = 0xDAB001;
pub const CURRENT_VERSION: u32 = 1;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryRange {
    start: u32,
    end: u32,
}

const impl Default for MemoryRange {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

impl MemoryRange {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub fn to_range(&self) -> Range<u32> {
        self.start..self.end
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct PayloadParams {
    /// Payload magic. Should be `MAGIC`
    pub magic: u32,
    /// Version. Should be `CURRENT_VERSION`
    pub version: u32,
    /// Device memory range
    pub memory: MemoryRange,
    /// Memory ranges which shouldn't be picked or overwritten
    pub blacklist: [BlacklistRange; 8],
    /// USB download function pointer
    pub ptr_dl: u32,
    /// USB upload function pointer
    pub ptr_ul: u32,
    /// Current SoC
    pub soc: SoC,
}

const impl Default for PayloadParams {
    fn default() -> Self {
        Self {
            magic: MAGIC,
            version: CURRENT_VERSION,
            memory: Default::default(),
            blacklist: [Default::default(); 8],
            ptr_dl: 0,
            ptr_ul: 0,
            soc: SoC::MT6572,
        }
    }
}

impl PayloadParams {
    pub const fn new(memory: Range<u32>, ptr_dl: u32, ptr_ul: u32, soc: SoC) -> Self {
        Self {
            magic: MAGIC,
            version: CURRENT_VERSION,
            memory: MemoryRange::new(memory.start, memory.end),
            blacklist: [Default::default(); 8],
            ptr_dl,
            ptr_ul,
            soc,
        }
    }

    fn find_free_range(&mut self) -> Option<&mut BlacklistRange> {
        self.blacklist
            .iter_mut()
            .find(|i| i.mode == BlacklistMode::None)
    }

    /// Blacklist memory range from the download
    pub fn blacklist_dl(&mut self, range: Range<u32>) -> Result<()> {
        let slot = self
            .find_free_range()
            .ok_or(err::Error::BlacklistExhausted)?;
        slot.range = MemoryRange::new(range.start, range.end);
        slot.mode = BlacklistMode::ForbiddenDL;
        Ok(())
    }

    /// Blacklist memory range from the relocation
    pub fn blacklist_reloc(&mut self, range: Range<u32>) -> Result<()> {
        let slot = self
            .find_free_range()
            .ok_or(err::Error::BlacklistExhausted)?;
        slot.range = MemoryRange::new(range.start, range.end);
        slot.mode = BlacklistMode::ForbiddenReloc;
        Ok(())
    }

    /// Select usable memory range with `size`
    pub fn find_unused_range(&self, size: u32) -> Option<Range<u32>> {
        // align to 8
        let aligned_size = size.checked_add(7)? & !7;

        let mut addr = self.memory.start;
        while addr.saturating_add(aligned_size) <= self.memory.end {
            let mut bad = false;

            for block in self.blacklist.iter() {
                if block.mode != BlacklistMode::None {
                    let r = block.range;

                    if addr < r.end && (addr + aligned_size) > r.start {
                        addr = r.end.checked_add(7)? & !7;
                        bad = true;
                        break;
                    }
                }
            }

            if !bad {
                return Some(addr..addr + aligned_size);
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BlacklistRange {
    pub range: MemoryRange,
    pub mode: BlacklistMode,
}

const impl Default for BlacklistRange {
    fn default() -> Self {
        Self::new(Default::default(), Default::default())
    }
}

impl BlacklistRange {
    pub const fn new(range: MemoryRange, mode: BlacklistMode) -> Self {
        Self { range, mode }
    }

    pub fn to_range(&self) -> Range<u32> {
        self.range.to_range()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum BlacklistMode {
    /// The range is available
    None,
    /// The range can't be used for the payload relocation
    ForbiddenReloc,
    /// The range can't be used for downloading data
    ForbiddenDL,
}

const impl Default for BlacklistMode {
    fn default() -> Self {
        Self::None
    }
}
