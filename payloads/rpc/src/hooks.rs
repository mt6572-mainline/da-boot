use crate::{LK_PARAMS, c_function};
use core::ptr;
use interceptor::{Interceptor, hook};
use shared::{Serial, uart_print, uart_println};

pub mod hooks {
    use super::*;

    hook! {
        fn mt_part_generic_read(ctx: InvocationContext) {
            let src = (ctx.r3 as u64) << 32 | ctx.r2 as u64;
            let dst = unsafe { *ctx.sp() } as *mut u8;
            let size = unsafe { *ctx.sp().add(1) } as usize;

            let Some(ref params) = LK_PARAMS else {
                panic!("LK parameters are not valid");
            };

            let mt_part_get_partition = params.ptr_mt_part_get_partition;
            let part = unsafe { c_function!(fn(*const u8) -> *const u32, mt_part_get_partition as usize | 1)(b"BOOTIMG\0".as_ptr()) };

            if !part.is_null() {
                let addr = unsafe { (*part.add(3) as u64) << 9 };
                let delta = (src - addr) as usize;

                if delta <= 0x1000 {
                    uart_println!("replacing boot.img");

                    unsafe {
                        ptr::copy_nonoverlapping((params.bootimg_scratch_addr as *const u8).add(delta), dst, size);
                    }

                    ctx.r0 = size as u32;
                    return;
                }
            }

            let ret = unsafe { c_function!(fn(u32, u32, u64, *mut u8, u32) -> u32, mt_part_generic_read::original() as usize | 1)
                (ctx.r0, 0, src, dst, size as u32) };
            ctx.r0 = ret;
        }
    }
}
