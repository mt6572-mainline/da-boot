use derive_ctor::ctor;

use crate::reader::Reader;

#[derive(ctor)]
pub struct Thumb2Reader {
    ptr: *const u16,
}

impl Thumb2Reader {
    /// Read u16 without consuming it
    #[inline(always)]
    pub unsafe fn poke16(&mut self) -> u16 {
        unsafe { Reader::read16(self.ptr) }
    }

    /// Read u32 without consuming it
    #[inline(always)]
    pub unsafe fn poke32(&mut self) -> u32 {
        unsafe { Reader::read32(self.ptr) }
    }

    /// Read u16
    #[inline(always)]
    pub unsafe fn read16(&mut self) -> u16 {
        unsafe {
            let v = self.poke16();
            self.skip(1);
            v
        }
    }

    /// Read u32
    #[inline(always)]
    pub unsafe fn read32(&mut self) -> u32 {
        unsafe {
            let v = self.poke32();
            self.skip(2);
            v
        }
    }

    #[inline(always)]
    pub unsafe fn skip(&mut self, v: usize) {
        unsafe { self.ptr = self.ptr.add(v) };
    }

    #[inline(always)]
    pub fn ptr(&self) -> *const u16 {
        self.ptr
    }

    /// Guess if the instruction is 32-bit wide, without consuming it
    #[inline]
    pub fn is_32bit(&self) -> bool {
        let v = unsafe { Reader::read16(self.ptr) } >> 11;
        v == 0b11101 || v == 0b11110 || v == 0b11111
    }

    /// Guess if the instruction is `ldr`
    #[inline]
    pub fn is_ldr(&mut self) -> bool {
        (unsafe { self.poke16() } & 0xF800) == 0x4800
    }

    #[inline]
    pub fn read_ldr(&mut self) -> RegAndImm<u32> {
        let v = unsafe { self.read16() };
        let r = ((v >> 8) & 7) as u8;
        let imm = ((v & 0xFF) as u32) << 2;

        RegAndImm { r, imm }
    }
}

pub struct RegAndImm<T = u16> {
    pub r: u8,
    pub imm: T,
}
