pub struct Reader;

impl Reader {
    /// Read `ptr` as u16
    #[inline(always)]
    pub unsafe fn read16(ptr: *const u16) -> u16 {
        unsafe { *ptr }
    }

    /// Read `ptr` as u32
    #[inline(always)]
    pub unsafe fn read32_unchecked(ptr: *const u32) -> u32 {
        unsafe { *(ptr as *const u32) }
    }
}
