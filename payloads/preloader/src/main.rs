#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    hint::unreachable_unchecked,
    mem::transmute,
    panic::PanicInfo,
    ptr,
};

const MT6572_UART0_LSR: usize = 0x11005000 + 0x14;
const MT6572_UART0_THR: usize = 0x11005000;

const PRELOADER_BASE: usize = 0x2007500;
const PRELOADER_END: usize = PRELOADER_BASE + 0x10000;

const DLCOMPORT_PTR: usize = 0x2000828;

/* Cortex-A7 cache line size */
const CACHE_LINE: usize = 64;

global_asm!(include_str!("start.S"));

macro_rules! uart_print {
    ($s:expr) => {{
        for c in $s.chars() {
            uart_putc(c);
        }
    }};
}

macro_rules! uart_println {
    ($s:expr) => {{
        uart_print!($s);
        uart_putc('\n');
        uart_putc('\r');
    }};
}

macro_rules! patch {
    ($addr:expr, $val:expr) => {{
        ptr::write_volatile($addr as *mut u16, $val);
    }};
}

macro_rules! search {
    ($start:expr, $end:expr, $( $pat:expr ),+ $(,)?) => {{
        const PATTERN: &[u16] = &[$($pat),+];
        crate::search_pattern($start, $end, PATTERN)
    }};
}

macro_rules! status {
    ($desc:literal, $code:expr) => {{
        uart_print!($desc);
        uart_print!(" is ");
        if let Err(_) = $code {
            uart_print!("NOT ");
        }
        uart_println!("patched");
    }};
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

unsafe fn flush_cache(addr: usize) {
    let addr = addr & !(CACHE_LINE - 1);

    unsafe {
        asm!("mcr p15, 0, {addr}, c7, c14, 1", addr = in(reg) addr, options(nostack, nomem));
        asm!("dsb");
        asm!("mcr p15, 0, r0, c7, c5, 0", options(nomem, nostack));
        asm!("isb");
    }
}

#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    uart_println!("Panic :(");
    loop {}
}

fn uart_putc(c: char) {
    unsafe {
        while (ptr::read_volatile(MT6572_UART0_LSR as *const u32) & 0x20) == 0 {}
        ptr::write_volatile(MT6572_UART0_THR as *mut u32, c as u32);
    }
}

unsafe fn is_movs(addr: usize) -> bool {
    (unsafe { ptr::read_volatile(addr as *const u16) } & 0xf800) == 0x2000
}

unsafe fn is_str_sp_rel(addr: usize) -> bool {
    (unsafe { ptr::read_volatile(addr as *const u16) } & 0xf800) == 0x9000
}

unsafe fn flip_str_to_ldr(addr: usize) {
    unsafe {
        ptr::write_volatile(
            addr as *mut u16,
            ptr::read_volatile(addr as *const u16) | (1 << 11),
        )
    }
}

unsafe fn extract_ldr_offset(addr: usize) -> usize {
    ((unsafe { ptr::read_volatile(addr as *const u16) } & 0xff) * 4) as usize
}

type UsbdlHandler = unsafe extern "C" fn(u32, u32);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    uart_println!("");
    uart_println!("Hello from Rust :)");

    uart_print!("usbdl_handler is ");
    let addr = if let Some(addr) = search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x4ef0, 0x460e) {
        uart_println!("found");
        addr
    } else {
        uart_println!("not found :(");
        panic!();
    };
    let usbdl_handler: UsbdlHandler = unsafe { transmute(addr | 1) };

    status!("send_da", {
        // mov r3, r0
        search!(addr, addr + 0x200, 0x4603)
            .map(|mut addr| unsafe {
                addr -= 8; // skip 32 bit instructions
                loop {
                    addr -= 2;

                    if is_str_sp_rel(addr) {
                        break;
                    }
                }

                flip_str_to_ldr(addr);
                flush_cache(addr);
            })
            .ok_or(())
    });

    status!("jump_da", {
        search!(PRELOADER_BASE, PRELOADER_END, 0x2600, 0x4630)
            .map(|mut addr| unsafe {
                addr += 40;

                // some preloaders may overwrite the DA with boot argument
                if is_movs(addr + 6) {
                    for i in 0..13 {
                        patch!(addr + 2 + (i * 2), 0xbf00); // nop
                    }
                } else {
                    for i in 0..7 {
                        patch!(addr + 2 + (i * 2), 0xbf00); // nop
                    }
                }
                flush_cache(addr);

                ptr::write_volatile(
                    (addr + extract_ldr_offset(addr) + 2) as *mut u32,
                    0x800d0000,
                );
            })
            .ok_or(())
    });

    status!("sec_region_check", {
        search!(PRELOADER_BASE, PRELOADER_END, 0xb537, 0x4604, 0x460d)
            .map(|addr| unsafe {
                patch!(addr, 0x2000); // movs r0, #0
                patch!(addr + 2, 0x4770); // bx lr
                flush_cache(addr);
            })
            .ok_or(())
    });

    uart_println!("Jumping to usbdl_handler...");
    unsafe {
        asm!("dsb; isb");
        usbdl_handler(ptr::read_volatile(DLCOMPORT_PTR as *const u32), 300);
        unreachable_unchecked();
    }
}
