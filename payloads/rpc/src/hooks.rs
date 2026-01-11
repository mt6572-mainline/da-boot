use crate::{get_boot_linux, get_boot_linux_from_storage};
use interceptor::{Interceptor, c_function, hook};
use shared::{LK_BASE, Serial, uart_print, uart_println};

pub mod hooks {

    use super::*;

    hook! {
        fn boot_linux_from_storage() {
            uart_println!("Booting linux");
            unsafe {
                c_function!(fn (usize, usize, usize, i32, usize, i32) -> !, get_boot_linux() | 1)(0x80108000, 0x80100100, b"console=ttyMT0,921600\0".as_ptr() as usize, 6572, 0x84100000, 0);
            }
        }
    }

    hook! {
        fn bldr_jump(addr: u32, arg1: u32, arg2: u32) {
            uart_println!("bldr_jump!!!!!!!");
            if addr == LK_BASE as u32 {
                unsafe { boot_linux_from_storage::replace(get_boot_linux_from_storage() | 1) };
            }

            unsafe {
                c_function!(fn(u32, u32, u32) -> !, bldr_jump::original() as usize | 1)(addr, arg1, arg2);
            }
        }
    }
}
