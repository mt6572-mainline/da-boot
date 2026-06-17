use crate::{LK_PARAMS, c_function, uart_println};
use core::ptr;
use interceptor::hook;

pub mod hooks {
    use core::ffi::c_void;

    use super::*;

    hook! {
        fn mt_part_generic_read(dev: *mut c_void, src: u64, dst: *mut u8, size: u32) -> u32 {
            let Some(ref params) = LK_PARAMS else {
                panic!("LK parameters are not valid");
            };

            let mt_part_get_partition = params.ptr_mt_part_get_partition;
            let f = unsafe { c_function!(fn(*const u8) -> *const u32, mt_part_get_partition as usize | 1) };
            let mut part = unsafe { f(b"BOOTIMG\0".as_ptr()) };
            let mut offset = 12;

            // new devices have boot instead of BOOTIMG
            if part.is_null() {
                part = unsafe { f(b"boot\0".as_ptr()) };
                offset = 0;
            }

            if !part.is_null() {
                let addr = unsafe { (*part.cast::<u8>().add(offset).cast::<u32>() as u64) << 9 };
                let delta = (src - addr) as u32;

                if delta <= 0x1000 {
                    uart_println!("replacing boot.img");

                    unsafe {
                        ptr::copy_nonoverlapping((params.bootimg_scratch_addr + delta) as _, dst, size as usize);
                    }

                    return size;
                }
            }

            return unsafe { c_function!(fn(*mut c_void, u64, *mut u8, u32) -> u32, mt_part_generic_read::original() as usize | 1)(dev, src, dst, size) };
        }
    }
}
