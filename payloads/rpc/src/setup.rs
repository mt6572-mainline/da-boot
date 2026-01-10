use core::arch::asm;

#[unsafe(no_mangle)]
pub extern "C" fn is_bootrom() -> bool {
    let pc: usize;
    unsafe { asm!("mov {}, pc", out(reg) pc) };
    pc < 0x80000000
}
