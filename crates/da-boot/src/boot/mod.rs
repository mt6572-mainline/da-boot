pub mod bootrom;
pub mod preloader;
pub mod rpc;

pub(super) fn give_me_bytes_please<'a, T: Sized>(v: &'a T) -> &'a [u8] {
    unsafe { core::slice::from_raw_parts(v as *const T as *const u8, core::mem::size_of::<T>()) }
}
