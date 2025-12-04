#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    hint::unreachable_unchecked,
    mem::transmute,
    panic::PanicInfo,
    ptr,
};

use bump::BumpAllocator;
use heapless::String;
use interceptor::{Interceptor, c_function, hook};
use shared::{LK_BASE, PRELOADER_BASE, flush_cache, search, search_pattern, uart_print, uart_println, uart_putc};
use ufmt::uwrite;

const PRELOADER_END: usize = PRELOADER_BASE + 0x10000;
const DA_PATCHER_PRELOADER_SIZE: u32 = 10 * 1024;

const LK_END: usize = LK_BASE + 0x100000;
const LK_KERNEL_ADDR: usize = 0x80108000;

const MAGIC: u32 = 0xDEADC0DE;

const DLCOMPORT_PTR: usize = 0x2000828;

global_asm!(include_str!("start.S"));

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new(0xa0000000);

macro_rules! status {
    ($desc:literal, $code:expr) => {{
        uart_print!($desc);
        uart_print!(" is ");
        if let Some(addr) = $code {
            uart_println!("found");
            addr
        } else {
            uart_println!("NOT found");
            panic!();
        }
    }};
}

macro_rules! dl_fn {
    ($name:ident, $ty:ty, $len:expr) => {
        unsafe fn $name(addr: usize) -> $ty {
            let mut buf = [0u8; $len];
            let ptr: UsbRecv = unsafe { transmute(addr | 1) };
            // ptr, len, timeout (ms)
            if unsafe { ptr(buf.as_mut_ptr(), $len, 0) } != 0 {
                uart_println!("usb_recv failed");
                panic!();
            }
            <$ty>::from_be_bytes(buf)
        }
    };
}

dl_fn!(dl8, u8, 1);
dl_fn!(dl32, u32, 4);

macro_rules! uart_printfln {
    ($s:expr, $fmt:literal $(, $($arg:tt)+)?) => {{
        uwrite!($s, $fmt $(, $($arg)+)?).unwrap();
        uart_println!($s);
        $s.clear();
    }};
}

#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    uart_println!("Panic :(");
    loop {}
}

mod hooks {
    use super::*;

    hook! {
        fn boot_linux_from_storage() {
            uart_println!("Booting linux");
            let addr = status!("boot_linux", { search!(LK_BASE, LK_END, 0xe92d, 0x4ff0, 0x2401, 0x460e, 0xf2c5) });
            let cmdline = status!("cmdline", { search!(LK_BASE, LK_END, 0x6f63, 0x736e, 0x6c6f, 0x3d65, 0x7474) }); // oc, sn, lo, =e, tt
            unsafe {
                c_function!(fn (usize, usize, usize, i32, usize, i32) -> !, addr | 1)(LK_KERNEL_ADDR, 0x80100100, cmdline, 6572, 0x84100000, 0);
            }
        }
    }

    hook! {
        fn bldr_jump(addr: u32, arg1: u32, arg2: u32) {
            uart_println!("bldr_jump");
            if addr == LK_BASE as u32 && unsafe { (LK_KERNEL_ADDR as *mut u32).read() } != MAGIC {
                let addr = status!("boot_linux_from_storage", { search!(LK_BASE, LK_END, 0xe92d, 0x41f0, 0x2000, 0xB082) });
                unsafe { boot_linux_from_storage::replace(addr | 1) };
            }

            unsafe {
                c_function!(fn(u32, u32, u32) -> !, bldr_jump::original() as usize | 1)(addr, arg1, arg2);
            }
        }
    }
}

type UsbdlHandler = unsafe extern "C" fn(u32, u32);
/// usb_send(u8* buf, u32 size);
type UsbSend = unsafe extern "C" fn(*const u8, u32);
/// usb_recv(u8* buf, u32 size, u32 timeout);
type UsbRecv = unsafe extern "C" fn(*mut u8, u32, u32) -> i32;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    uart_println!("");
    uart_println!("Hello from Rust :)");

    let addr = status!("usbdl_handler", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x4ef0, 0x460e) });
    let usbdl_handler: UsbdlHandler = unsafe { transmute(addr | 1) };

    let addr = status!("usb_send", { search!(PRELOADER_BASE, PRELOADER_END, 0xb508, 0x4603, 0x2200, 0x4608, 0x4619) });
    let usb_send: UsbSend = unsafe { transmute(addr | 1) };

    let addr = status!("usb_recv", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x42f0, 0x4605, 0x2000) });
    let usb_recv: UsbRecv = unsafe { transmute(addr | 1) };

    let bldr_jump = status!("bldr_jump", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x46f8, 0x4691, 0x4604) });
    unsafe {
        Interceptor::init();
        hooks::bldr_jump::replace(bldr_jump | 1);
        (LK_KERNEL_ADDR as *mut u32).write(MAGIC);
    }

    let mut s = String::<64>::new();

    uart_println!("Ready for commands...");
    loop {
        let command = unsafe { dl8(addr) };
        unsafe { usb_send([command].as_ptr(), 1) };
        uart_printfln!(s, "Got 0x{:x}", command);

        match command {
            // patch
            0x01 => unsafe {
                let da_addr = dl32(addr);
                let len = dl32(addr);

                uart_printfln!(s, "Patching 0x{:x}..0x{:x}...", da_addr, da_addr + len);

                usb_recv(da_addr as *mut u8, len, 0);
                uart_print!("flush...");
                flush_cache(da_addr as usize, len as usize);
                uart_println!("ok");
            },
            // dump preloader
            0x02 => unsafe {
                uart_print!("Dumping preloader...");
                usb_send(DA_PATCHER_PRELOADER_SIZE.to_be_bytes().as_ptr(), 4);
                usb_send(PRELOADER_BASE as *const u8, DA_PATCHER_PRELOADER_SIZE);
                uart_println!("ok");
            },
            // jump back
            0x03 => unsafe {
                uart_println!("Jumping to usbdl_handler...");
                asm!("dsb; isb");
                usbdl_handler(ptr::read_volatile(DLCOMPORT_PTR as *const u32), 300);
                unreachable_unchecked();
            },
            _ => uart_println!("Unknown command"),
        }
    }
}
