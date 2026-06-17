use core::ptr;

use acon::MMIO;
use ufmt::uWrite;

use crate::setup::get_soc;

pub struct Serial;

impl Serial {
    pub fn putc(c: u8) {
        let mmio = get_soc().uart0();

        unsafe {
            while (ptr::read_volatile((mmio + 0x14) as *const u32) & 0x20) == 0 {}
            ptr::write_volatile((mmio + 0x00) as *mut u32, c as u32);
        }
    }
}

impl uWrite for Serial {
    type Error = core::convert::Infallible;

    fn write_str(&mut self, s: &str) -> Result<(), Self::Error> {
        for c in s.as_bytes() {
            Self::putc(*c);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! uart_print {
    ($s:expr) => {{
        for c in $s.bytes() {
            crate::uart::Serial::putc(c);
        }
    }};
}

#[macro_export]
macro_rules! uart_println {
    ($s:expr) => {{
        crate::uart_print!($s);
        crate::uart::Serial::putc(b'\n');
        crate::uart::Serial::putc(b'\r');
    }};
}

#[macro_export]
macro_rules! uart_printfln {
    ($fmt:literal $(, $($arg:tt)+)?) => {{

        ufmt::uwrite!(&mut crate::uart::Serial, $fmt $(, $($arg)+)?);
        crate::uart_println!("");
    }};
}
