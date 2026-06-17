#![no_std]
#![no_main]
#![feature(const_trait_impl)]
#![allow(static_mut_refs)]

use da_protocol::{LKRunnerParams, PreloaderRunnerParams};

use crate::{
    setup::{banner, die},
    usb::handler,
};
use core::panic::PanicInfo;

mod err;
mod macros;
mod setup;
mod uart;
mod usb;

static mut PRELOADER_PARAMS: Option<PreloaderRunnerParams> = None;
static mut LK_PARAMS: Option<LKRunnerParams> = None;

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    uart_println!("Panic :(");

    loop {}
}

unsafe fn main() {
    banner();

    uart_println!("start usb");
    unsafe { handler() };
}
