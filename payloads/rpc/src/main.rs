#![no_std]
#![no_main]

use bump::BumpAllocator;
use core::{
    arch::{asm, global_asm},
    mem::transmute,
    panic::PanicInfo,
    ptr,
};
use da_protocol::{Message, Protocol, ProtocolError, Response};
use derive_ctor::ctor;
use interceptor::c_function;
use shared::{PRELOADER_BASE, Serial, flush_cache, search, search_pattern, uart_print, uart_println};
use simpleport::{SimpleRead, SimpleWrite};
use ufmt::{uWrite, uwrite};

use crate::setup::is_bootrom;

mod setup;

const USBDL_PUT_DATA: usize = 0x40BA4A;
const USBDL_GET_DATA: usize = 0x40B9C4;

const PRELOADER_END: usize = PRELOADER_BASE + 0x10000;
const DLCOMPORT_PTR: usize = 0x2000828;

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new(0xa0000000);

global_asm!(include_str!("start.S"));

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
    ($fmt:literal $(, $($arg:tt)+)?) => {{
        uwrite!(&mut Serial, $fmt $(, $($arg)+)?);
        uart_println!("");
    }};
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

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    uart_println!("Panic :(");

    if let Some(message) = info.message().as_str() {
        uart_printfln!("Message: {}", message);
    }

    if let Some(location) = info.location() {
        uart_printfln!("{}: {}", location.file(), location.line());
    }

    Serial::disable_fifo();
    loop {}
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    Serial::enable_fifo();
    uart_println!("");
    uart_println!("Hello from Rust :)");

    let usb = if is_bootrom() {
        unsafe { USB::new(transmute(USBDL_GET_DATA | 1), transmute(USBDL_PUT_DATA | 1)) }
    } else {
        let send_addr = status!("usb_send", { search!(PRELOADER_BASE, PRELOADER_END, 0xb508, 0x4603, 0x2200, 0x4608, 0x4619) });
        let recv_addr = status!("usb_recv", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x42f0, 0x4605, 0x2000) });

        unsafe { USB::new(transmute(recv_addr | 1), transmute(send_addr | 1)) }
    };

    let buf = [0; 2048];
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
            Ok(message) => {
                Serial::putc(message.debug());
                match message {
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
                    Message::Jump { addr, r0, r1 } => unsafe {
                        Serial::disable_fifo();
                        if is_bootrom() {
                            asm!("dsb; isb");
                            c_function!(fn(u32, u32), addr as usize)(r0.unwrap_or_default(), r1.unwrap_or_default());
                        } else {
                            let bldr_jump = status!("bldr_jump", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x46f8, 0x4691, 0x4604) });
                            asm!("dsb; isb");
                            c_function!(fn(u32, u32, u32), bldr_jump | 1)(addr, r0.unwrap_or_default(), r1.unwrap_or_default());
                        }
                        Response::nack(ProtocolError::unreachable())
                    },
                    Message::Reset => unsafe {
                        Serial::disable_fifo();
                        (0x10007014 as *mut u32).write_volatile(0x1209);
                        Response::ack()
                    },
                    Message::Return => unsafe {
                        Serial::disable_fifo();
                        if is_bootrom() {
                            Response::nack(ProtocolError::not_supported())
                        } else {
                            let usbdl_handler_addr = status!("usbdl_handler", { search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x4ef0, 0x460e) });

                            asm!("dsb; isb");
                            c_function!(fn(u32, u32) -> (), usbdl_handler_addr | 1)(ptr::read_volatile(DLCOMPORT_PTR as *const u32), 300);
                            Response::nack(ProtocolError::unreachable())
                        }
                    },
                }
            }
            Err(e) => {
                uart_println!("Error reading message");
                Response::nack(ProtocolError::unreachable())
            }
        };

        Serial::putc(response.debug());
        if let Err(e) = protocol.send_response(response) {
            uart_println!("Error sending response, giving up");
            panic!();
        }
    }
}
