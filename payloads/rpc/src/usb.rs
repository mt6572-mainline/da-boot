use core::{arch::asm, convert::Infallible, mem::transmute};

use acon::MMIO;
use da_protocol::{HookId, Message, ParamsType, Protocol, ProtocolError, Response};
use derive_ctor::ctor;
use shared::flush_cache;
use simpleport::{SimpleRead, SimpleWrite};

use crate::{
    LK_PARAMS, PRELOADER_PARAMS, c_function, die,
    setup::{get_params, get_params_mut, get_soc, is_bootrom},
    uart_printfln, uart_println,
};

#[cfg(feature = "pl")]
use crate::hooks::hooks;

#[derive(ctor)]
pub struct USB {
    recv: unsafe extern "C" fn(*mut u8, u32, u32) -> u32,
    send: unsafe extern "C" fn(*const u8, u32),
}
impl SimpleRead for USB {
    type Error = Infallible;

    fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        unsafe { (self.recv)(buf.as_mut_ptr(), buf.len() as u32, 0) };
        Ok(())
    }
}

impl SimpleWrite for USB {
    type Error = Infallible;

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        unsafe { (self.send)(buf.as_ptr(), buf.len() as u32) };
        Ok(())
    }
}

pub unsafe fn handler() -> ! {
    let params = get_params();
    let usb = unsafe { USB::new(transmute(params.ptr_dl as usize), transmute(params.ptr_ul as usize)) };
    let mut protocol = Protocol::new(usb);

    uart_println!("send");
    if protocol.send_message(Message::Ack).is_err() {
        die("failed to send ack");
    }

    uart_println!("wait");
    if let Ok(r) = protocol.read_response()
        && r.is_ack()
    {
        uart_println!("Ready for commands");
    } else {
        die("got invalid ack");
    }

    loop {
        let response = match protocol.read_message() {
            Ok(message) => match message {
                Message::Ack => Response::Ack,
                Message::Read { addr, size } => unsafe {
                    let data = core::slice::from_raw_parts(addr as *const u8, size as usize);
                    protocol.io.write(data);
                    Response::Ack
                },
                Message::Write { addr, size } => unsafe {
                    let data = core::slice::from_raw_parts_mut(addr as *mut u8, size as usize);
                    protocol.io.read(data);
                    uart_printfln!("read {:#x} bytes to {:#x}", size, addr);
                    Response::Ack
                },
                Message::FlushCache { addr, size } => unsafe {
                    flush_cache(addr as usize, size as usize);
                    Response::Ack
                },
                Message::Jump { addr, r0, r1 } => unsafe {
                    if is_bootrom() {
                        asm!("dsb; isb");
                        c_function!(fn(u32, u32), addr as usize)(r0.unwrap_or_default(), r1.unwrap_or_default());
                        Response::Nack(ProtocolError::Unreachable)
                    } else {
                        if let Some(ref params) = PRELOADER_PARAMS {
                            asm!("dsb; isb");
                            c_function!(fn(u32, u32, u32), params.ptr_bldr_jump as usize | 1)(addr, r0.unwrap_or_default(), r1.unwrap_or_default());
                            Response::Nack(ProtocolError::Unreachable)
                        } else {
                            Response::Nack(ProtocolError::InvalidParams)
                        }
                    }
                },
                Message::Reset => unsafe {
                    ((get_soc().toprgu() + 0x14) as *mut u32).write_volatile(0x1209);
                    Response::ack()
                },
                Message::Hook(id) => {
                    #[cfg(feature = "pl")]
                    match id {
                        HookId::MtPartGenericRead => unsafe {
                            if let Some(ref params) = LK_PARAMS {
                                hooks::mt_part_generic_read::replace(params.ptr_mt_part_generic_read as usize | 1);
                                uart_println!("replaced mt_part_generic_read");
                                Response::Ack
                            } else {
                                Response::Nack(ProtocolError::InvalidParams)
                            }
                        },
                    }
                    #[cfg(not(feature = "pl"))]
                    Response::Nack(ProtocolError::NotSupported)
                }
                Message::GetFreeRange { size } => Response::Range(get_params().find_unused_range(size).map(|r| r.start)),
                Message::BlacklistRange(range) => {
                    if get_params_mut().blacklist_dl(range).is_ok() {
                        Response::Ack
                    } else {
                        Response::Nack(ProtocolError::NotSupported)
                    }
                }
                Message::SetParams(params) => unsafe {
                    if is_bootrom() {
                        Response::Nack(ProtocolError::NotSupported)
                    } else {
                        match params {
                            ParamsType::Preloader(pl) => {
                                if pl.is_valid() {
                                    PRELOADER_PARAMS = Some(pl);
                                    Response::Ack
                                } else {
                                    Response::Nack(ProtocolError::InvalidParams)
                                }
                            }
                            ParamsType::LK(lk) => {
                                if lk.is_valid() {
                                    LK_PARAMS = Some(lk);
                                    Response::Ack
                                } else {
                                    Response::Nack(ProtocolError::InvalidParams)
                                }
                            }
                        }
                    }
                },
            },
            Err(e) => {
                uart_println!("Error reading message");
                Response::nack(ProtocolError::Unreachable)
            }
        };

        if let Err(e) = protocol.send_response(response) {
            die("Error sending response, giving up");
        }
    }
}
