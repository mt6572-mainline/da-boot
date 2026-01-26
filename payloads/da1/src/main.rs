#![no_std]
#![no_main]

use core::{arch::global_asm, mem::transmute, panic::PanicInfo, ptr};

use shared::{Serial, flush_icache, uart_print, uart_println};

#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    uart_println!("Panic :(");
    loop {}
}

global_asm!(
    ".syntax unified
        .code 32

        .global start
        .section .text.start
        start:
            mov r0, pc
            blx main"
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main(pc: usize) -> ! {
    let start = pc - (4 * 5) - 8; // 5 instructions + pipeline

    unsafe {
        let cmp_addr = ptr::read_volatile(start as *const *mut u16); // cmp instruction address
        let ptr = ptr::read_volatile((start - 4) as *const *mut u32); // croissant2 fn ptr
        let original_fn = ptr::read_volatile((start - 8) as *const u32); // croissant2 fn ptr value, 'p' in the pumpkin mode
        let jump = ptr::read_volatile((start - 12) as *const usize); // function to receive and boot da2

        // Croissant2 mode: restore fn ptr
        if (original_fn & 0xff) as u8 != b'p' {
            ptr.write_volatile(original_fn);
        }

        cmp_addr.write_volatile(0x4289); // cmp r1, r1 to pass the check
        flush_icache();

        Serial::putc(b'j');
        type Boot = unsafe extern "C" fn() -> !;
        let f: Boot = transmute(jump);
        f();
    }
}
