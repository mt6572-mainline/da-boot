#![no_std]

use core::ptr;

use shared::flush_cache;

use crate::{code::JUMP, err::Error};

mod code;
pub mod err;

pub type Result<T> = core::result::Result<T, Error>;

#[macro_export]
macro_rules! hook {
    (
        fn $name:ident() $body:block
    ) => {
        mod $name {
            use super::*;

            #[unsafe(naked)]
            #[unsafe(no_mangle)]
            unsafe extern "C" fn thunk() {
                core::arch::naked_asm!(
                    "push {{r4-r11, lr}}",
                    "bl body",
                    "pop {{r4-r11, lr}}",
                    "bx lr",
                );
            }

            #[unsafe(no_mangle)]
            extern "C" fn body() {
                $body
            }

            pub unsafe fn replace(target: *mut u8) -> interceptor::Result<()> {
                unsafe { Interceptor::replace(target, thunk as *mut u8) }
            }
        }
    };
}

pub struct Interceptor;

impl Interceptor {
    pub unsafe fn replace(target: *mut u8, replacement: *mut u8) -> Result<()> {
        if target as usize & 1 == 0 {
            return Err(Error::UnsupportedMode);
        }

        let target = (target as usize & !1) as *mut u8;
        unsafe {
            ptr::write_volatile(target as *mut u32, JUMP);
            ptr::write_volatile(target.add(4) as *mut u32, replacement as u32);

            flush_cache(target as usize);
        }

        Ok(())
    }
}
