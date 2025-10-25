#![no_std]

use core::ptr;

pub const PRELOADER_BASE: usize = 0x2007500;

const UART0_LSR: usize = 0x11005000 + 0x14;
const UART0_THR: usize = 0x11005000;

pub fn uart_putc(c: u8) {
    unsafe {
        while (ptr::read_volatile(UART0_LSR as *const u32) & 0x20) == 0 {}
        ptr::write_volatile(UART0_THR as *mut u32, c as u32);
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
