#![no_std]
#![no_main]

use bump::BumpAllocator;
use core::{
    arch::{asm, global_asm},
    mem::transmute,
    panic::PanicInfo,
    ptr,
};
use da_protocol::{Message, NotFoundError, Property, Protocol, ProtocolError, Response};
use derive_ctor::ctor;
use interceptor::{Interceptor, c_function};
use shared::{LK_BASE, PRELOADER_BASE, Serial, flush_cache, search, search_pattern, uart_print, uart_println};
use simpleport::{SimpleRead, SimpleWrite};

use crate::{hooks::BOOT_IMG, setup::is_bootrom};

mod hooks;
mod setup;

const USBDL_PUT_DATA: usize = 0x40BA4A;
const USBDL_GET_DATA: usize = 0x40B9C4;

const PRELOADER_END: usize = PRELOADER_BASE + 0x10000;
const DLCOMPORT_PTR: usize = 0x2000828;

const LK_END: usize = LK_BASE + 0x100000;

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new(0x90000000);

global_asm!(include_str!("start.S"));

#[macro_export]
macro_rules! uart_printfln {
    ($fmt:literal $(, $($arg:tt)+)?) => {{
        ufmt::uwrite!(&mut Serial, $fmt $(, $($arg)+)?);
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

    Serial::disable_fifo();
    loop {}
}

fn get_bldr_jump() -> Option<usize> {
    search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x46f8, 0x4691, 0x4604).or_else(|| search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x46f8, 0x4607, 0x4692))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    Serial::enable_fifo();
    uart_println!("");
    uart_println!("Hello from Rust :)");

    let usb = if is_bootrom() {
        unsafe { USB::new(transmute(USBDL_GET_DATA | 1), transmute(USBDL_PUT_DATA | 1)) }
    } else {
        let send_addr = search!(PRELOADER_BASE, PRELOADER_END, 0xb508, 0x4603, 0x2200, 0x4608, 0x4619).expect("usb_send not found");
        let recv_addr = search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x42f0, 0x4605, 0x2000).expect("usb_recv not found");

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
                            Response::nack(ProtocolError::Unreachable)
                        } else {
                            if let Some(bldr_jump) = get_bldr_jump() {
                                asm!("dsb; isb");
                                c_function!(fn(u32, u32, u32), bldr_jump | 1)(addr, r0.unwrap_or_default(), r1.unwrap_or_default());
                                Response::nack(ProtocolError::Unreachable)
                            } else {
                                Response::nack(ProtocolError::NotFound(NotFoundError::BldrJump))
                            }
                        }
                    },
                    Message::GetProperty(property) => match property {
                        Property::BootImgAddress => Response::value(BOOT_IMG),
                    },
                    Message::Reset => unsafe {
                        Serial::disable_fifo();
                        (0x10007014 as *mut u32).write_volatile(0x1209);
                        Response::ack()
                    },
                    Message::LKHook => unsafe {
                        uart_println!("Initializing interceptor");
                        Interceptor::init();

                        if let Some(mt_part_generic_read) =
                            search!(LK_BASE, LK_END, 0xe92d, 0x4ff0, 0x4699, 0x4b60, 0xb08d).or_else(|| search!(LK_BASE, LK_END, 0xe92d, 0x4ff0, 0x4699, 0x4b61, 0xb089, 0x4690))
                        {
                            hooks::hooks::mt_part_generic_read::replace(mt_part_generic_read | 1);
                            uart_println!("replaced mt_part_generic_read");
                            Response::ack()
                        } else {
                            Response::nack(ProtocolError::NotFound(NotFoundError::MtPartGenericRead))
                        }
                    },
                    Message::Return => unsafe {
                        Serial::disable_fifo();
                        if is_bootrom() {
                            Response::nack(ProtocolError::NotSupported)
                        } else {
                            if let Some(usbdl_handler_addr) = search!(PRELOADER_BASE, PRELOADER_END, 0xe92d, 0x4ef0, 0x460e) {
                                asm!("dsb; isb");
                                c_function!(fn(u32, u32) -> (), usbdl_handler_addr | 1)(ptr::read_volatile(DLCOMPORT_PTR as *const u32), 300);
                                Response::nack(ProtocolError::Unreachable)
                            } else {
                                Response::nack(ProtocolError::NotFound(NotFoundError::UsbDlHandler))
                            }
                        }
                    },
                }
            }
            Err(e) => {
                uart_println!("Error reading message");
                Response::nack(ProtocolError::Unreachable)
            }
        };

        Serial::putc(response.debug());
        if let Err(e) = protocol.send_response(response) {
            uart_println!("Error sending response, giving up");
            panic!();
        }
    }
}
