use crate::LK_END;
use core::ptr;
use interceptor::{Interceptor, c_function, hook};
use shared::{LK_BASE, Serial, search, uart_print, uart_println};

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

    hook! {
        fn mboot_android_check_img_info(ctx: InvocationContext) {
            let original = c_function!(fn(*const u8, *mut u8) -> i32, mboot_android_check_img_info::original() as usize | 1);
            let name = ctx.r0 as *const u8;

            let ret = unsafe { original(name, ctx.r1 as _) };
            if ret < 0 {
                ctx.r0 = unsafe { original(b"RECOVERY\0".as_ptr(), ctx.r1 as _) } as u32;
                uart_println!("supplied recovery image, workaround applied");
            } else {
                ctx.r0 = ret as u32;
            }
        }
    }
}
