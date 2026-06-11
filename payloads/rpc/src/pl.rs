#![no_std]
#![no_main]
#![feature(const_trait_impl)]
#![allow(static_mut_refs)]

use bump::BumpAllocator;
use da_protocol::{LKRunnerParams, PreloaderRunnerParams};
use shared::{Serial, uart_print, uart_println};

use crate::{
    setup::{banner, die, get_params, get_params_mut},
    usb::handler,
};
use core::panic::PanicInfo;

mod err;
mod exception;
mod hooks;
mod macros;
mod setup;
mod usb;

const HEAP_SIZE: usize = 1 * 1024 * 1024;

#[global_allocator]
static mut ALLOCATOR: BumpAllocator = BumpAllocator::empty();

static mut PRELOADER_PARAMS: Option<PreloaderRunnerParams> = None;
static mut LK_PARAMS: Option<LKRunnerParams> = None;

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    uart_println!("Panic :(");

    loop {}
}

unsafe fn main() {
    banner();
    let Some(heap) = get_params().find_unused_range(HEAP_SIZE as u32) else {
        die("failed to create heap");
    };
    unsafe { ALLOCATOR.init(heap.start as usize, HEAP_SIZE) };
    uart_printfln!("heap initialized at {:#x} with {:#x} bytes", heap.start, HEAP_SIZE);
    if get_params_mut().blacklist_dl(heap).is_err() {
        die("unable to blacklist the heap");
    }

    uart_println!("start usb");
    unsafe { handler() };
}
