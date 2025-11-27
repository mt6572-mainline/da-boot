use derive_ctor::ctor;

pub const JUMP: u32 = 0xf000_f8df; // ldr.w pc, [pc, #0]
pub const NOP: u16 = 0xbf00;

#[derive(ctor)]
pub struct RegAndValue {
    pub r: u8,
    pub value: u32,
}

#[derive(ctor)]
pub struct Branch {
    pub value: u32,
}

pub fn is_32bit(v: u16) -> bool {
    let v = v >> 11;
    v == 0b11101 || v == 0b11110 || v == 0b11111
}

pub fn is_ldr(v: u16) -> bool {
    v & 0xF800 == 0x4800
}

pub fn extract_ldr(v: u16) -> RegAndValue {
    let r = ((v >> 8) & 7) as u8;
    let imm = ((v & 0xFF) as u32) << 2;

    RegAndValue::new(r, imm)
}

pub fn is_adr(v: u16) -> bool {
    v & 0xF800 == 0xA000
}

pub fn extract_adr(v: u16) -> RegAndValue {
    let r = ((v >> 8) & 7) as u8;
    let imm = ((v & 0xFF) as u32) << 2;

    RegAndValue::new(r, imm)
}

pub fn is_b(v: u16) -> bool {
    v & 0xF000 == 0xD000 || {
        let cond = (v >> 8) & 0xF;
        cond == 0xF
    }
}

pub fn extract_b(v: u16) -> Branch {
    let imm = (v & 0xFF) << 1;

    Branch::new(imm as u32)
}

pub fn is_ldr_w(v: u32) -> bool {
    v & 0xFF7F0000 == 0xF85F0000
}

pub fn extract_ldr_w(v: u32) -> RegAndValue {
    let r = ((v >> 12) & 0xF) as u8;
    let imm = (v & 0xFFF) as u32;

    RegAndValue::new(r, imm)
}

pub fn is_b_w(v: u32) -> bool {
    v & 0xF8008000 == 0xF0008000
}

pub fn extract_b_w(v: u32) -> Branch {
    let imm11 = ((v >> 0) & 0x7FF) as u32;
    let imm10 = ((v >> 16) & 0x3FF) as u32;
    let s = ((v >> 26) & 1) as u32;
    let j1 = ((v >> 13) & 1) as u32;
    let j2 = ((v >> 11) & 1) as u32;

    let i1 = !(j1 ^ s) & 1;
    let i2 = !(j2 ^ s) & 1;

    let imm = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);

    let imm = (imm << 7) >> 7;

    Branch::new(imm)
}

pub fn is_blx(v: u32) -> bool {
    v & 0xF800D000 == 0xF000D000
}

pub fn extract_blx(v: u32) -> Branch {
    let imm11 = ((v >> 0) & 0x7FF) as u32;
    let imm10 = ((v >> 16) & 0x3FF) as u32;
    let s = ((v >> 26) & 1) as u32;
    let j1 = ((v >> 13) & 1) as u32;
    let j2 = ((v >> 11) & 1) as u32;

    let i1 = !(j1 ^ s) & 1;
    let i2 = !(j2 ^ s) & 1;

    let imm = (s << 24) | (i1 << 23) | (i2 << 22) | (imm10 << 12) | (imm11 << 1);

    let imm = (imm << 7) >> 7;

    Branch::new(imm)
}

fn pack_movw(rd: u8, imm: u16) -> u32 {
    let rd = rd as u32;
    let imm = imm as u32;

    let imm4 = (imm >> 12) & 0xF;
    let i = (imm >> 11) & 1;
    let imm3 = (imm >> 8) & 0x7;
    let imm8 = imm & 0xFF;

    let hw1 = 0xF240 | (i << 10) | imm4;

    let hw2 = (imm3 << 12) | (rd << 8) | imm8;

    (hw1 << 16) | hw2
}

fn pack_movt(rd: u8, imm: u16) -> u32 {
    let rd = rd as u32;
    let imm = imm as u32;

    let imm4 = (imm >> 12) & 0xF;
    let i = (imm >> 11) & 1;
    let imm3 = (imm >> 8) & 0x7;
    let imm8 = imm & 0xFF;

    let hw1 = 0xF2C0 | (i << 10) | imm4;

    let hw2 = (imm3 << 12) | (rd << 8) | imm8;

    (hw1 << 16) | hw2
}

pub fn pack_mov_pair(r: u8, v: u32) -> (u32, u32) {
    let l = (v & 0xFFFF) as u16;
    let u = (v >> 16) as u16;

    let instr_movw = pack_movw(r, l);
    let instr_movt = pack_movt(r, u);

    (instr_movw, instr_movt)
}

pub unsafe fn write_thumb2_instr(ptr: *mut u8, w: u32) {
    let u = (w >> 16) as u16;
    let l = w as u16;

    unsafe { core::ptr::write_unaligned(ptr as *mut u16, u) };
    unsafe { core::ptr::write_unaligned(ptr.add(2) as *mut u16, l) };
}
