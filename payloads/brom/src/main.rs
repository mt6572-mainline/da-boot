#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    mem::transmute,
    panic::PanicInfo,
    ptr::{self, null_mut},
};

use da_protocol::{Message, Protocol, Response};
use derive_ctor::ctor;
use interceptor::c_function;
use shared::{flush_cache, uart_print, uart_println, uart_putc};
use simpleport::{SimpleRead, SimpleWrite};

const WATCHDOG: usize = 0x10007000;

const SEND_USB_RESPONSE: usize = 0x406ac8;
const USBDL_PUT_DATA: usize = 0x40BA4A;
const USBDL_GET_DATA: usize = 0x40B9C4;

global_asm!(include_str!("start.S"));

#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    uart_println!("Panic :(");
    loop {}
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

type SendUsbResponse = unsafe extern "C" fn(u32, u32, u32);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    // disable watchdog or the game is over
    unsafe {
        ptr::write_volatile(WATCHDOG as *mut u32, 0x22000064);
    }
    uart_println!("");
    uart_println!("Hello from Rust :)");

    let buf = [0; 2048];
    let usb = unsafe { USB::new(transmute(USBDL_GET_DATA | 1), transmute(USBDL_PUT_DATA | 1)) };
    let mut protocol = Protocol::new(usb, buf);

    if protocol.send_message(&Message::ack()).is_err() {
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
                    c_function!(fn(u32, u32), addr as usize)(
                        r1.unwrap_or_default(),
                        r2.unwrap_or_default(),
                    );
                    Response::nack()
                },
                Message::Reset => unsafe {
                    (0x10007014 as *mut u32).write_volatile(0x1209);
                    Response::ack()
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
