use crate::LK_END;
use core::ptr;
use interceptor::{Interceptor, c_function, hook};
use shared::{LK_BASE, Serial, flush_cache, search, search_pattern, uart_print, uart_println};

pub const BOOT_IMG: u32 = 0x83000000;

pub mod hooks {

    use super::*;

    hook! {
        fn mt_part_generic_read(ctx: InvocationContext) {
            let src = (ctx.r3 as u64) << 32 | ctx.r2 as u64;
            let dst = unsafe { *ctx.sp() } as *mut u8;
            let size = unsafe { *ctx.sp().add(1) } as usize;

            let mt_part_get_partition = search!(LK_BASE, LK_END, 0xe92d, 0x41f0, 0x4607, 0x4920, 0x463a).or_else(|| search!(LK_BASE, LK_END, 0x4b26, 0x4602, 0x4926, 0xe92d, 0x41f0)).expect("mt_part_get_partition not found");
            let part = unsafe { c_function!(fn(*const u8) -> *const u32, mt_part_get_partition | 1)(b"BOOTIMG\0".as_ptr()) };

            if !part.is_null() {
                let addr = unsafe { (*part.add(3) as u64) << 9 };
                    let delta = (src - addr) as usize;

                if delta <= 0x1000 {
                    uart_println!("replacing boot.img");

                    unsafe {
                        ptr::copy_nonoverlapping((BOOT_IMG as *const u8).add(delta), dst, size);

                        flush_cache(dst as usize, size);
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
