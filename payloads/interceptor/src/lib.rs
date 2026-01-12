#![no_std]
#![allow(static_mut_refs)]

#[cfg(feature = "alloc")]
extern crate alloc;

use core::{alloc::Layout, mem::MaybeUninit, ptr};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use shared::flush_cache;

use crate::{
    code::{JUMP, NOP, extract_ldr, is_32bit, is_ldr, pack_mov_pair, write_thumb2_instr},
    err::Error,
};

mod code;
pub mod err;

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
macro_rules! c_function {
    (fn ($($a:ty),*) $(-> $r:ty)?, $addr:expr) => {
        unsafe {
            type F = unsafe extern "C" fn ($($a),*) $(-> $r)?;
            let f: F = core::mem::transmute($addr);
            f
        }
    };
}

#[macro_export]
macro_rules! hook {
    (
        fn $name:ident($ctx_name:ident: $ctxty:ty) $(-> $ret:ty)? $body:block
    ) => {
        pub mod $name {
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
                    sym body
                );
            }

            pub(super) unsafe extern "C" fn body(ctx: &mut interceptor::InvocationContext) $(-> $ret)? {
                let $ctx_name = ctx;
                $body
            }

            pub unsafe fn replace(target: usize) -> interceptor::Result<()> {
                unsafe {
                    Interceptor::replace(target, thunk)?;
                    ADDR = target;
                }
                Ok(())
            }

            pub unsafe fn revert() -> interceptor::Result<()> {
                unsafe {
                    Interceptor::revert(ADDR)?;
                    ADDR = 0;
                }
                Ok(())
            }

            pub unsafe fn original() -> *mut u8 {
                unsafe { Interceptor::original(ADDR) }.unwrap_or(ADDR) as *mut u8
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

    unsafe fn place_jump(ptr: *mut u8, to: u32) {
        unsafe {
            (ptr as *mut u32).write_volatile(JUMP);
            (ptr.add(4) as *mut u32).write_volatile(to);
        }
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

        let mut target_ptr = Self::unmask_thumb2(target) as *mut u8;

        #[cfg(feature = "alloc")]
        let pool = unsafe { POOL.get_mut() };

        let size = if target as u32 % 4 != 0 { 10 } else { 8 };

        #[cfg(feature = "alloc")]
        // worst case
        let layout = unsafe { Layout::from_size_align_unchecked(64, 4) };

        #[cfg(feature = "alloc")]
        unsafe {
            let code = alloc::alloc::alloc(layout);
            let mut n_target = 0;
            let mut n_code = 0;
            while n_target < size {
                let v = (target_ptr.add(n_target) as *const u16).read_unaligned();
                if is_32bit(v) {
                    // just copy for now
                    ptr::copy_nonoverlapping(target_ptr.add(n_target), code.add(n_code), 4);
                    n_target += 4;
                    n_code += 4;
                } else {
                    if is_ldr(v) {
                        let data = extract_ldr(v);
                        let pc_aligned = (target_ptr as u32 + n_target as u32 + 4) & !3;
                        let literal_address = pc_aligned + data.imm;
                        let value = ptr::read_volatile(literal_address as *const u32);
                        let (movw, movt) = pack_mov_pair(data.r, value);
                        write_thumb2_instr(code.add(n_code), movw);
                        write_thumb2_instr(code.add(n_code + 4), movt);

                        n_code += 8;
                    } else {
                        // just copy for now
                        ptr::copy_nonoverlapping(target_ptr.add(n_target), code.add(n_code), 2);
                        n_code += 2;
                    }

                    n_target += 2;
                }
            }

            while (code as u32 + n_code as u32) % 4 != 0 {
                ptr::write_volatile(code.add(n_code) as *mut u16, NOP);
                n_code += 2;
            }

            let jump_address = (target as u32 + n_target as u32) | 1;

            ptr::write_volatile(code.add(n_code) as *mut u32, JUMP);
            ptr::write_volatile(code.add(n_code + 4) as *mut u32, jump_address);

            flush_cache(code as usize, 64);

            pool.trampoline.push(Trampoline {
                address: code as u32,
                jump_address,
            });
        };

        #[cfg(feature = "alloc")]
        pool.address.push(target_ptr as u32);

        unsafe {
            if target_ptr as u32 % 4 != 0 {
                ptr::write_volatile(target_ptr as *mut u16, NOP);
                target_ptr = target_ptr.add(2);
            }

            Self::place_jump(target_ptr, replacement as u32 | 1);
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
