#[macro_export]
macro_rules! c_function {
    (fn ($($a:ty),*) $(-> $r:ty)?, $addr:expr) => {{
        type F = unsafe extern "C" fn ($($a),*) $(-> $r)?;
        let f: F = core::mem::transmute($addr);
        f
    }};
}
