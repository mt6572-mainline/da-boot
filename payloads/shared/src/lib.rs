#![no_std]

use core::{arch::asm, ptr};

pub const PRELOADER_BASE: usize = 0x2007500;

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
    let end_addr = (start_addr + size + CACHE_LINE - 1) & !(CACHE_LINE - 1);

    unsafe {
        // flush data
        let mut addr = start_addr;
        while addr < end_addr {
            asm!("mcr p15, 0, {addr}, c7, c14, 1", addr = in(reg) addr, options(nostack, nomem));
            addr += CACHE_LINE;
        }

        asm!("dsb");

        // flush instructions
        asm!("mcr p15, 0, r0, c7, c5, 0", options(nomem, nostack));

        // flush bp
        asm!("mcr p15, 0, r0, c7, c5, 6", options(nomem, nostack));

        asm!("dsb");
        asm!("isb");
    }
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
