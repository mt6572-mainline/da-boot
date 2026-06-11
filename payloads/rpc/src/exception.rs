use core::arch::global_asm;

use shared::{Serial, uart_print, uart_println};

global_asm!(
    "
.code 32
.section .vectors, \"ax\"
.global vector_table
.align 5

vector_table:
    b start
    b undef_handler
    b unk_handler
    b prefetch_abort_handler
    b data_abort_handler
    b unk_handler
    b unk_handler
    b unk_handler

undef_handler:
    sub lr, lr, #4
    mov r0, lr                @ pc
    mov r1, #0                @ fault addr
    mov r2, #0                @ status
    cpsid iF
    b undef_abort_handler_rust
    b .

prefetch_abort_handler:
    sub lr, lr, #4
    mov r0, lr                @ pc
    mrc p15, 0, r1, c6, c0, 2 @ fault addr
    mrc p15, 0, r2, c5, c0, 1 @ status
    cpsid iF
    b prefetch_abort_handler_rust
    b .

data_abort_handler:
    sub lr, lr, #8
    mov r0, lr                @ pc
    mrc p15, 0, r1, c6, c0, 0 @ fault addr
    mrc p15, 0, r2, c5, c0, 0 @ status
    cpsid iF
    b data_abort_handler_rust
    b .

unk_handler:
    cpsid iF
    adr sp, _exception_stack_end
    bl unknown_handler
    b .

.align 3
    _exception_stack_start:
        .space 32
    _exception_stack_end:
"
);

macro_rules! handler {
    ($name:ident, $readable_name:literal) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(pc: u32, fault_addr: u32, status: u32) -> ! {
            uart_print!("got ");
            uart_println!($readable_name);
            unsafe { exception_handler(pc, fault_addr, status) };
        }
    };
}

handler!(undef_abort_handler_rust, "undef abort");
handler!(prefetch_abort_handler_rust, "prefetch abort");
handler!(data_abort_handler_rust, "data abort");

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exception_handler(pc: u32, fault_addr: u32, status: u32) -> ! {
    uart_println!("EXCEPTION:");

    uart_print!("died at: ");
    print_hex(pc);
    uart_println!("");

    uart_print!("fault addr: ");
    print_hex(fault_addr);
    uart_println!("");

    uart_print!("status: ");
    print_hex(status);
    uart_println!("");

    loop {}
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn unknown_handler() -> ! {
    uart_println!("unexpected handler entered. this shouldn't have happened");

    loop {}
}

fn print_hex(v: u32) {
    uart_print!("0x");

    for i in (0..8).rev() {
        let n = (v >> (i * 4)) & 0xF;

        let c = if n < 10 { b'0' + n as u8 } else { b'A' + (n - 10) as u8 };

        Serial::putc(c);
    }
}
