#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    mem::transmute,
    panic::PanicInfo,
    ptr::{self, null_mut},
};

use shared::{uart_print, uart_println, uart_putc};

const WATCHDOG: usize = 0x10007000;

const SEND_USB_RESPONSE: usize = 0x406ac8;
const USBDL_PUT_WORD: usize = 0x40B90A;
const USBDL_GET_DWORD: usize = 0x40B94E;

global_asm!(include_str!("start.S"));

#[panic_handler]
fn panic_handler(_: &PanicInfo) -> ! {
    uart_println!("Panic :(");
    loop {}
}

type Entry = unsafe extern "C" fn();
type SendUsbResponse = unsafe extern "C" fn(u32, u32, u32);
type UsbDlPutWord = unsafe extern "C" fn(u16, u32);
type UsbDlGetDword = unsafe extern "C" fn(*mut u32) -> u32;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn main() -> ! {
    // disable watchdog or the game is over
    unsafe {
        ptr::write_volatile(WATCHDOG as *mut u32, 0x22000064);
    }
    uart_println!("");
    uart_println!("Hello from Rust :)");

    let send_usb_response: SendUsbResponse = unsafe { transmute(SEND_USB_RESPONSE | 1) };
    unsafe {
        send_usb_response(1, 0, 1);
    }

    let usbdl_put_word: UsbDlPutWord = unsafe { transmute(USBDL_PUT_WORD | 1) };
    let usbdl_get_dword: UsbDlGetDword = unsafe { transmute(USBDL_GET_DWORD | 1) };

    unsafe {
        uart_println!("step 1: ack");
        let ack = usbdl_get_dword(null_mut());
        if ack != 0x1337 {
            uart_println!("wrong ack");
            panic!();
        }
        usbdl_put_word(0x1337, 1);

        uart_println!("step 2: dl");
        let addr = usbdl_get_dword(null_mut());
        let len = usbdl_get_dword(null_mut());
        for i in 0..(len / 4) {
            let dword = u32::from_be_bytes(usbdl_get_dword(null_mut()).to_le_bytes());
            ptr::write_volatile((addr + (i * 4)) as *mut u32, dword);
        }
        usbdl_put_word(0, 1);

        uart_println!("step 3: jmp");
        asm!("dsb; isb");
        let entry: Entry = transmute(addr as usize);
        entry();
    }
    uart_println!("failed");

    loop {}
}
