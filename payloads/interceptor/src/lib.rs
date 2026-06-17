#![no_std]
#![allow(static_mut_refs)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{alloc::Layout, mem::MaybeUninit, ptr};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use shared::flush_cache;

use crate::{err::Error, thumb2writer::Thumb2Writer};

pub mod err;
mod reader;
mod thumb2reader;
mod thumb2writer;
mod writer;

pub type Result<T> = core::result::Result<T, Error>;

#[cfg(feature = "alloc")]
struct Static<T>(MaybeUninit<T>);

#[cfg(feature = "alloc")]
impl<T> Static<T> {
    unsafe fn init(&mut self, t: T) {
        self.0.write(t);
    }

    unsafe fn get(&self) -> &T {
        unsafe { &*self.0.as_ptr() }
    }

    unsafe fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.0.as_mut_ptr() }
    }
}

#[macro_export]
macro_rules! hook_with_context {
    ($mod_name:ident, $body_fn:path) => {
        pub mod $mod_name {
            #[allow(unused_imports)]
            use super::*;

            #[allow(static_mut_refs)]
            static mut ADDR: usize = 0;

            #[unsafe(naked)]
            pub(super) unsafe extern "C" fn thunk() {
                core::arch::naked_asm!(
                    "push {{r0-r3}}",
                    "push {{r4-r11, r12, lr}}",
                    "mov r0, sp",
                    "bl {}",
                    "pop {{r4-r11, r12, lr}}",
                    "pop {{r0-r3}}",
                    "bx lr",
                    sym $body_fn
                );
            }

            pub unsafe fn replace(target: usize) -> $crate::Result<()> {
                unsafe {
                    $crate::Interceptor::replace(target, thunk)?;
                    ADDR = target;
                }
                Ok(())
            }

            pub unsafe fn revert() -> $crate::Result<()> {
                unsafe {
                    $crate::Interceptor::revert(ADDR)?;
                    ADDR = 0;
                }
                Ok(())
            }

            pub unsafe fn original() -> *mut u8 {
                unsafe { $crate::Interceptor::original(ADDR) }.unwrap_or(ADDR) as *mut u8
            }
        }
    };
}

#[macro_export]
macro_rules! hook {
    (
        fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block
    ) => {
        pub mod $name {
            use super::*;

            #[allow(static_mut_refs)]
            static mut ADDR: usize = 0;

            pub unsafe extern "C" fn body($($arg: $ty),*) $(-> $ret)? {
                $body
            }

            pub unsafe fn original() -> unsafe extern "C" fn($($ty),*) $(-> $ret)? {
                let addr = unsafe { $crate::Interceptor::original(ADDR) }.unwrap_or(ADDR);
                core::mem::transmute(addr)
            }

            pub unsafe fn replace(target: usize) -> $crate::Result<()> {
                unsafe {
                    let f: unsafe extern "C" fn() = core::mem::transmute(body as *const ());
                    $crate::Interceptor::replace(target, f)?;
                    ADDR = target;
                }
                Ok(())
            }

            pub unsafe fn revert() -> $crate::Result<()> {
                unsafe {
                    $crate::Interceptor::revert(ADDR)?;
                    ADDR = 0;
                }
                Ok(())
            }
        }
    };
}

#[repr(C)]
pub struct InvocationContext {
    pub r4: u32,
    pub r5: u32,
    pub r6: u32,
    pub r7: u32,
    pub r8: u32,
    pub r9: u32,
    pub r10: u32,
    pub r11: u32,
    pub r12: u32,
    pub lr: u32,

    pub r0: u32,
    pub r1: u32,
    pub r2: u32,
    pub r3: u32,
}

impl InvocationContext {
    pub unsafe fn sp(&self) -> *const u32 {
        unsafe { (self as *const _ as *const u8).add(size_of::<Self>()) as *const u32 }
    }
}

#[cfg(feature = "alloc")]
struct Trampoline {
    address: u32,
    jump_address: u32,
}

#[cfg(feature = "alloc")]
struct InterceptorPool {
    address: Vec<u32>,
    trampoline: Vec<Trampoline>,
}

#[cfg(feature = "alloc")]
static mut POOL: Static<InterceptorPool> = Static(MaybeUninit::zeroed());

pub struct Interceptor;

impl Interceptor {
    const fn unmask_thumb2(addr: usize) -> usize {
        addr & !1
    }

    #[cfg(feature = "alloc")]
    pub unsafe fn init() {
        unsafe {
            POOL.init(InterceptorPool {
                address: Vec::with_capacity(10),
                trampoline: Vec::with_capacity(10),
            });
        }
    }

    pub unsafe fn replace(target: usize, replacement: unsafe extern "C" fn()) -> Result<()> {
        if target & 1 == 0 {
            return Err(Error::UnsupportedMode);
        }

        let target_ptr = Self::unmask_thumb2(target) as *mut u8;

        #[cfg(feature = "alloc")]
        let pool = unsafe { POOL.get_mut() };

        let size = if target as u32 % 4 != 0 { 10 } else { 8 };

        #[cfg(feature = "alloc")]
        // worst case
        let layout = unsafe { Layout::from_size_align_unchecked(64, 4) };

        #[cfg(feature = "alloc")]
        unsafe {
            use crate::{reader::Reader, thumb2reader::Thumb2Reader};

            let code = alloc::alloc::alloc(layout);
            let mut n_target = 0;

            let mut reader = Thumb2Reader::new(target_ptr as *const u16);
            let mut writer = Thumb2Writer::new(code as *mut u16);

            if !writer.is_aligned32() {
                writer.nop();
            }

            while n_target < size {
                if reader.is_32bit() {
                    if reader.is_ldr_w() {
                        let data = reader.read_ldr_w();

                        let pc_aligned = (target_ptr as u32 + n_target as u32 + 4) & !3;
                        let literal_address = if data.add {
                            pc_aligned + data.imm
                        } else {
                            pc_aligned - data.imm
                        };
                        let value = Reader::read32_unchecked(literal_address as *const u32);

                        writer.movw(data.r, (value & 0xFFFF) as u16);
                        writer.movt(data.r, (value >> 16) as u16);
                    } else {
                        // just copy for now
                        writer.copy(reader.ptr() as *const u8, 4);
                        reader.skip(2);
                    }

                    n_target += 4;
                } else {
                    if reader.is_ldr() {
                        let data = reader.read_ldr();
                        let pc_aligned = (target_ptr as u32 + n_target as u32 + 4) & !3;
                        let literal_address = pc_aligned + data.imm;
                        let value = Reader::read32_unchecked(literal_address as *const u32);

                        let l = (value & 0xFFFF) as u16;
                        let u = (value >> 16) as u16;
                        writer.movw(data.r, l);
                        writer.movt(data.r, u);
                    } else {
                        // just copy for now
                        writer.write16(reader.read16());
                    }

                    n_target += 2;
                }
            }

            if !writer.is_aligned32() {
                writer.nop();
            }

            let jump_address = reader.ptr() as u32 | 1;
            writer.jumpout(jump_address);

            flush_cache(code as usize, 64);

            pool.trampoline.push(Trampoline {
                address: code as u32,
                jump_address,
            });
        };

        #[cfg(feature = "alloc")]
        pool.address.push(target_ptr as u32);

        unsafe {
            let mut writer = Thumb2Writer::new(target_ptr as *mut u16);
            if !writer.is_aligned32() {
                writer.nop();
            }

            writer.jumpout(replacement as u32 | 1);
            flush_cache(target, 64);
        }

        Ok(())
    }

    #[cfg(feature = "alloc")]
    pub unsafe fn revert(target: usize) -> Result<()> {
        if target & 1 == 0 {
            return Err(Error::UnsupportedMode);
        }

        let pool = unsafe { POOL.get_mut() };
        let target = Self::unmask_thumb2(target);
        let (i, trampoline) = pool
            .address
            .iter()
            .position(|addr| *addr == target as _)
            .map(|i| (i, &pool.trampoline[i]))
            .ok_or(Error::TrampolineNotFound)?;

        unsafe {
            let size = trampoline.jump_address as usize - target;

            ptr::copy_nonoverlapping(trampoline.address as *const u8, target as *mut u8, size);
            flush_cache(target, size);
        }

        pool.address.remove(i);
        pool.trampoline.remove(i);

        Ok(())
    }

    #[cfg(feature = "alloc")]
    pub unsafe fn original(target: usize) -> Option<usize> {
        let pool = unsafe { POOL.get() };
        let target = Self::unmask_thumb2(target);
        pool.address
            .iter()
            .position(|addr| *addr == target as _)
            .map(|i| pool.trampoline[i].address as _)
    }
}
