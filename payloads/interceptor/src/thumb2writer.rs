use derive_ctor::ctor;

use crate::writer::Writer;

#[derive(ctor)]
pub struct Thumb2Writer {
    ptr: *mut u16,
}

impl Thumb2Writer {
    #[inline(always)]
    pub unsafe fn write16(&mut self, v: u16) {
        unsafe {
            Writer::write16(self.ptr, v);
            self.ptr = self.ptr.add(1);
        }
    }

    pub unsafe fn copy(&mut self, src: *const u8, size: usize) {
        unsafe {
            (self.ptr as *mut u8).copy_from_nonoverlapping(src, size);
            self.ptr = self.ptr.add(size / 2)
        }
    }

    /// Write `v` as 32-bit instruction
    ///
    /// This may emit additional `nop` if not aligned
    pub unsafe fn write32(&mut self, v: u32) {
        unsafe {
            if !self.is_aligned32() {
                self.nop();
            }

            Writer::write32_unchecked(self.ptr, v);
            self.ptr = self.ptr.add(2);
        }
    }

    #[inline(always)]
    pub unsafe fn is_aligned32(&self) -> bool {
        self.ptr as usize % 4 == 0
    }

    #[inline(always)]
    const fn movtw(is_movt: bool, r: u8, imm: u16) -> u32 {
        let r = r as u32;
        let imm = imm as u32;

        let imm4 = (imm >> 12) & 0xF;
        let i = (imm >> 11) & 1;
        let imm3 = (imm >> 8) & 0x7;
        let imm8 = imm & 0xFF;

        let hw1 = if is_movt { 0xF2C0 } else { 0xF240 } | (i << 10) | imm4;

        let hw2 = (imm3 << 12) | (r << 8) | imm8;

        (hw2 as u32) << 16 | (hw1 as u32)
    }

    /// Emit `ldr.w pc, [pc]; *addr*`
    pub unsafe fn jumpout(&mut self, addr: u32) {
        unsafe {
            self.write32(0xf000_f8df);
            self.write32(addr);
        }
    }

    /// Emit `nop`
    pub unsafe fn nop(&mut self) {
        unsafe { self.write16(0xbf00) };
    }

    /// Emit `movt r, #imm`
    pub unsafe fn movt(&mut self, r: u8, imm: u16) {
        unsafe { self.write32(Self::movtw(true, r, imm)) };
    }

    /// Emit `movw r, #imm`
    pub unsafe fn movw(&mut self, r: u8, imm: u16) {
        unsafe { self.write32(Self::movtw(false, r, imm)) };
    }
}
