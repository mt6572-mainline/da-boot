#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    mem::transmute,
    panic::PanicInfo,
    ptr,
};

use bump::BumpAllocator;
use da_protocol::{Message, Protocol, Response};
use derive_ctor::ctor;
use heapless::String;
use interceptor::{Interceptor, c_function, hook};
use shared::{LK_BASE, PRELOADER_BASE, flush_cache, search, search_pattern, uart_print, uart_println, uart_putc};
use simpleport::{SimpleRead, SimpleWrite};
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

#[derive(ctor)]
struct USB {
    recv: unsafe extern "C" fn(*mut u8, u32, u32) -> u32,
    send: unsafe extern "C" fn(*const u8, u32),
}
impl SimpleRead for USB {
    fn read(&mut self, buf: &mut [u8]) -> simpleport::Result<()> {
        unsafe { (self.recv)(buf.as_mut_ptr(), buf.len() as u32, 0) };
        Ok(())
    }
}

impl SimpleWrite for USB {
    fn write(&mut self, buf: &[u8]) -> simpleport::Result<()> {
        unsafe { (self.send)(buf.as_ptr(), buf.len() as u32) };
        Ok(())
    }
}

type UsbdlHandler = unsafe extern "C" fn(u32, u32);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    uart_println!("");
    uart_println!("Hello from Rust :)");

    let usbdl_handler_addr = status!("usbdl_handler", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x4ef0, 0x460e) });

    let send_addr = status!("usb_send", { search!(PRELOADER_BASE, PRELOADER_END, 0xb508, 0x4603, 0x2200, 0x4608, 0x4619) });

    let recv_addr = status!("usb_recv", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x42f0, 0x4605, 0x2000) });

    let bldr_jump = status!("bldr_jump", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x46f8, 0x4691, 0x4604) });

    unsafe {
        Interceptor::init();
        hooks::bldr_jump::replace(bldr_jump | 1);
        (LK_KERNEL_ADDR as *mut u32).write(MAGIC);
    }

    let buf = [0; 2048];
    let mut s = String::<64>::new();

    let usb = unsafe { USB::new(transmute(recv_addr | 1), transmute(send_addr | 1)) };
    let mut protocol = Protocol::new(usb, buf);

    if protocol.send_message(Message::ack()).is_err() {
        uart_println!("Failed to send ack");
        panic!();
    }

    if let Ok(r) = protocol.read_response()
        && r.is_ack()
    {
        uart_println!("Ready for commands");
    } else {
        uart_println!("Got invalid ack");
        panic!();
    }

    loop {
        let response = match protocol.read_message() {
            Ok(message) => match message {
                Message::Ack => Response::ack(),
                Message::Read { addr, size } => unsafe {
                    let data = core::slice::from_raw_parts(addr as *const u8, size as usize);
                    Response::read(data)
                },
                Message::Write { addr, data } => unsafe {
                    ptr::copy_nonoverlapping(data.as_ptr(), addr as _, data.len());
                    Response::ack()
                },
                Message::FlushCache { addr, size } => unsafe {
                    flush_cache(addr as usize, size as usize);
                    Response::ack()
                },
                Message::Jump { addr, r1, r2 } => unsafe {
                    asm!("dsb; isb");
                    c_function!(fn(u32, u32) -> (), addr as usize)(r1.unwrap_or_default(), r2.unwrap_or_default());
                    Response::nack()
                },
                Message::Reset => unsafe {
                    (0x10007014 as *mut u32).write_volatile(0x1209);
                    Response::ack()
                },
                Message::Return => unsafe {
                    asm!("dsb; isb");
                    c_function!(fn(u32, u32) -> (), usbdl_handler_addr | 1)(ptr::read_volatile(DLCOMPORT_PTR as *const u32), 300);
                    Response::nack()
                },
            },
            Err(e) => {
                uart_println!("Error reading message");
                Response::nack()
            }
        };

        if let Err(e) = protocol.send_response(response) {
            uart_println!("Error sending response, giving up");
            panic!();
        }
    }
}
