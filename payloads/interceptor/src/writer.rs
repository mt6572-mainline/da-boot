pub struct Writer;

impl Writer {
    /// Write to `ptr` u16
    #[inline(always)]
    pub unsafe fn write16(ptr: *mut u16, val: u16) {
        unsafe { ptr.write_volatile(val) };
    }

    /// Write to `ptr` u32
    #[inline(always)]
    pub unsafe fn write32_unchecked(ptr: *mut u16, val: u32) {
        unsafe { (ptr as *mut u32).write_volatile(val) };
    }
}
