#[macro_export]
macro_rules! uart_printfln {
    ($fmt:literal $(, $($arg:tt)+)?) => {{
        use shared::{Serial, uart_print, uart_println};
        ufmt::uwrite!(&mut Serial, $fmt $(, $($arg)+)?);
        uart_println!("");
    }};
}

#[macro_export]
macro_rules! c_function {
    (fn ($($a:ty),*) $(-> $r:ty)?, $addr:expr) => {{
        type F = unsafe extern "C" fn ($($a),*) $(-> $r)?;
        let f: F = core::mem::transmute($addr);
        f
    }};
}
