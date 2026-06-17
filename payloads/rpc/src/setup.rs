use core::{
    arch::{asm, global_asm},
    mem::MaybeUninit,
    ptr::{self, copy_nonoverlapping},
};

use acon::{MMIO, SoC};
use da_params::{BlacklistMode, CURRENT_VERSION, MAGIC, PayloadParams};
use shared::flush_icache;

use crate::{c_function, err::ParamsError, uart_print, uart_printfln, uart_println};

#[unsafe(link_section = ".params")]
pub static mut PARAMS: PayloadParams = PayloadParams::new(0..0, 0, 0);

pub static mut SOC: MaybeUninit<SoC> = MaybeUninit::uninit();

#[inline(always)]
pub fn banner() {
    uart_println!("");
    uart_printfln!("Hello from Rust and {} :)", get_soc());
}

#[inline(always)]
fn register_black_box(mut ptr: *const PayloadParams) -> *const PayloadParams {
    unsafe {
        asm!(
            "/* {} */",
            inout(reg) ptr,
            options(nostack, nomem, preserves_flags)
        );
    }
    ptr
}

#[inline(always)]
fn register_black_box_mut(mut ptr: *mut PayloadParams) -> *mut PayloadParams {
    unsafe {
        asm!(
            "/* {} */",
            inout(reg) ptr,
            options(nostack, nomem, preserves_flags)
        );
    }
    ptr
}

#[inline(always)]
pub unsafe fn where_am_i() -> usize {
    let pc: usize;
    unsafe { asm!("mov {}, pc", out(reg) pc) };
    pc
}

pub fn get_params() -> &'static PayloadParams {
    // this is so compiler doesn't optimize dummy struct
    unsafe {
        let params_ptr = register_black_box(&raw const PARAMS);
        &*params_ptr
    }
}

pub fn get_params_mut() -> &'static mut PayloadParams {
    unsafe {
        let params_ptr = register_black_box_mut(&raw mut PARAMS);
        &mut *params_ptr
    }
}

pub fn get_soc() -> &'static SoC {
    unsafe { SOC.assume_init_ref() }
}

#[inline(always)]
pub unsafe fn verify_params() -> Result<(), ParamsError> {
    unsafe {
        let pc = where_am_i() as u32;

        let params = get_params();
        if params.magic != MAGIC {
            Err(ParamsError::InvalidMagic)
        } else if params.version != CURRENT_VERSION {
            Err(ParamsError::InvalidVersion)
        } else if params.memory.to_range().is_empty() {
            Err(ParamsError::InvalidMemoryRange)
        } else if params.ptr_dl == 0 || params.ptr_ul == 0 {
            Err(ParamsError::InvalidFnPtr)
        } else if params
            .blacklist
            .iter()
            .any(|range| range.to_range().contains(&pc) && range.mode == BlacklistMode::ForbiddenReloc)
        {
            Err(ParamsError::RunningInBlacklistedRange)
        } else {
            Ok(())
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn is_bootrom() -> bool {
    unsafe { where_am_i() < 0x40000000 }
}

pub fn die(why: &str) -> ! {
    uart_print!("HALTED: ");
    uart_println!(why);

    loop {}
}

unsafe extern "C" {
    static _image_start: u32;
    static _rel_dyn_start: u32;
    static _rel_dyn_end: u32;
    static _bss_start: u32;
    static _bss_end: u32;
    static _image_end: u32;
}

#[repr(C)]
struct Elf32_Rel {
    r_offset: u32,
    r_info: u32,
}

const R_ARM_RELATIVE: u32 = 23;

#[inline(always)]
pub unsafe fn copy_and_jump(src: u32, addr: u32, size: u32) {
    let dst = addr as *mut u8;
    unsafe { copy_nonoverlapping(src as *const u8, dst, size as usize) };

    uart_println!("jump to relocated image");
    unsafe {
        flush_icache();
        c_function!(fn() -> !, addr as usize)();
    }
}

unsafe fn fix_got(runtime_base: u32) {
    let link_rel_start = &raw const _rel_dyn_start as u32;
    let link_rel_end = &raw const _rel_dyn_end as u32;

    // runtime addr
    let mut rel = (runtime_base + link_rel_start) as *const Elf32_Rel;
    let rel_end = (runtime_base + link_rel_end) as *const Elf32_Rel;

    let delta = runtime_base;

    // fix GOT
    while rel < rel_end {
        unsafe {
            let rel_type = (*rel).r_info & 0xFF;

            if rel_type == R_ARM_RELATIVE {
                let target_ptr = (runtime_base + (*rel).r_offset) as *mut u32;
                *target_ptr = (*target_ptr).wrapping_add(delta);
            }

            rel = rel.add(1);
        }
    }
}

// we need ARM entry but everything else should be Thumb
global_asm!(
    ".syntax unified
     .code 32

     .section .text.start
     .global start

     start:
         adr r0, start
         adr sp, _bootstrap_stack_end
         blx app

.align 3
     _bootstrap_stack_start:
         .space 128
     _bootstrap_stack_end:
"
);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn app(runtime_base: u32) -> ! {
    unsafe { SOC = MaybeUninit::new(SoC::try_from_mmio().unwrap()) };

    uart_printfln!("running at {:#x}", runtime_base);

    if let Err(e) = unsafe { verify_params() } {
        uart_printfln!("error on verifying params: {}", e);
    }

    let params = get_params();

    let start_ptr = &raw const _image_start as u32;
    let end_ptr = &raw const _image_end as u32;
    let size = end_ptr - start_ptr;

    let Some(reloc_range) = params.find_unused_range(size) else {
        die("failed to find relocation range");
    };

    let addr = reloc_range.start;
    if addr != runtime_base {
        unsafe { copy_and_jump(runtime_base, addr, size) };
    } else {
        unsafe { fix_got(runtime_base) };
        unsafe {
            let bss_start_addr = runtime_base + (&raw const _bss_start as u32);
            let bss_end_addr = runtime_base + (&raw const _bss_end as u32);
            let bss_size = bss_end_addr - bss_start_addr;

            ptr::write_bytes(bss_start_addr as *mut u8, 0, bss_size as usize);
        }

        // prevent host from overwriting image
        if get_params_mut().blacklist_dl(addr..addr + size).is_err() {
            die("failed to blacklist image");
        }

        // we need another range here, now for the stack
        let Some(stack_range) = params.find_unused_range(4 * 1024) else {
            die("can't find free memory range for the stack");
        };

        let addr = stack_range.end & !7;
        // prevent host from overwriting stack
        if get_params_mut().blacklist_dl(stack_range).is_err() {
            die("failed to blacklist stack");
        }

        uart_println!("jump to main");

        unsafe {
            core::arch::asm!(
                "mov sp, {stack}",
                "b {main}",
                stack = in(reg) addr,
                main = sym crate::main,
                options(noreturn),
            );
        }
    }

    die("reached end of the app");
}
