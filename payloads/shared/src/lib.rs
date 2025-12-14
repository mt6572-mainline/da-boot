#![no_std]
use core::{arch::asm, ptr};

pub const PRELOADER_BASE: usize = 0x2007500;
pub const LK_BASE: usize = 0x80020000;

const UART0_LSR: usize = 0x11005000 + 0x14;
const UART0_THR: usize = 0x11005000;

/* Cortex-A7 cache line size */
const CACHE_LINE: usize = 64;

pub fn uart_putc(c: u8) {
    unsafe {
        while (ptr::read_volatile(UART0_LSR as *const u32) & 0x20) == 0 {}
        ptr::write_volatile(UART0_THR as *mut u32, c as u32);
    }
}

pub unsafe fn flush_cache(start_addr: usize, size: usize) {
    let start_addr = start_addr & !(CACHE_LINE - 1);

    unsafe {
        flush_dcache(start_addr, size);
        flush_icache();
    }
}

pub unsafe fn flush_dcache(start_addr: usize, size: usize) {
    let end_addr = (start_addr + size + CACHE_LINE - 1) & !(CACHE_LINE - 1);

    let mut addr = start_addr;
    while addr < end_addr {
        unsafe {
            asm!("mcr p15, 0, {addr}, c7, c14, 1", addr = in(reg) addr, options(nostack, nomem))
        };
        addr += CACHE_LINE;
    }
    unsafe { asm!("dsb") };
}

pub unsafe fn flush_icache() {
    unsafe {
        asm!("mcr p15, 0, r0, c7, c5, 0", options(nomem, nostack));
        asm!("mcr p15, 0, r0, c7, c5, 6", options(nomem, nostack));

        asm!("dsb");
        asm!("isb");
    }
}

pub fn search_pattern(start: usize, end: usize, code: &[u16]) -> Option<usize> {
    let n = code.len();
    if n == 0 || end <= start {
        return None;
    }

    let end = end.saturating_sub(n * 2);

    let mut offset = start;
    while offset < end {
        // SAFETY: Thumb2 instructions are always readable as u16,
        // even if they're actually 32-bit wide
        let first = unsafe { *(offset as *const u16) };
        if first != code[0] {
            offset += 2;
            continue;
        }

        let mut matched = true;
        for i in 1..n {
            let check_addr = offset + (i * 2);
            let value = unsafe { *(check_addr as *const u16) };
            if value != code[i] {
                matched = false;
                break;
            }
        }

        if matched {
            return Some(offset);
        }

        offset += 2;
    }

    None
}

#[macro_export]
macro_rules! search {
    ($start:expr, $end:expr, $( $pat:expr ),+ $(,)?) => {{
        const PATTERN: &[u16] = &[$($pat),+];
        crate::search_pattern($start, $end, PATTERN)
    }};
}

#[macro_export]
macro_rules! uart_print {
    ($s:expr) => {{
        for c in $s.bytes() {
            uart_putc(c);
        }
    }};
}

#[macro_export]
macro_rules! uart_println {
    ($s:expr) => {{
        uart_print!($s);
        uart_putc(b'\n');
        uart_putc(b'\r');
    }};
}
