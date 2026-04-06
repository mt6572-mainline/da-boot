use yaxpeax_arm::armv7::{Opcode, Operand, Reg};

use crate::{Code, regext::RegExt};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Value {
    #[default]
    /// Unknown value
    Unknown,
    /// Constant
    Immediate(u32),
    /// Literal pool load at `base` with `offset`
    Load { base: u32, offset: i32 },
    /// Value from `base` with `offset` without derefencing
    RegisterOffset { r: Reg, offset: i32 },
    /// Dereference from `r` with `offset`
    Deref { r: Reg, offset: i32 },
}

impl Value {
    pub fn merge(&self, other: &Self) -> Self {
        if self == other {
            self.clone()
        } else {
            Value::Unknown
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct RegWriteTracker {
    regs: [Value; 16],
}

impl RegWriteTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_regs(regs: [Value; 16]) -> Self {
        Self { regs }
    }

    pub fn immediate(&mut self, reg: u8, value: u32) {
        self.regs[reg as usize] = Value::Immediate(value);
    }

    pub fn load(&mut self, reg: u8, value: u32) {
        self.regs[reg as usize] = Value::Load {
            base: value,
            offset: 0,
        };
    }

    pub fn register_offset(&mut self, reg: u8, r: Reg, offset: i32) {
        self.regs[reg as usize] = Value::RegisterOffset { r, offset }
    }

    pub fn deref(&mut self, reg: u8, r: Reg, offset: i32) {
        self.regs[reg as usize] = Value::Deref { r, offset };
    }

    pub fn call(&mut self) {
        for i in 0..=3 {
            self.regs[i] = Value::Unknown;
        }
    }

    pub fn get(&self, reg: u8) -> Value {
        self.regs[reg as usize].clone()
    }

    pub fn try_get_imm(&self, reg: u8, base_address: usize, data: &[u8]) -> Option<u32> {
        match self.get(reg) {
            Value::Unknown => None,
            Value::Immediate(imm) => Some(imm),
            Value::Load { base, offset } => {
                let load = (base as usize).checked_sub(base_address)?;
                let end = load + 4;

                if data.len() < end {
                    None
                } else {
                    let val = u32::from_le_bytes(data[load..end].try_into().unwrap());
                    Some(val.wrapping_add_signed(offset))
                }
            }
            Value::RegisterOffset { r, offset } => {
                if r.number() == reg {
                    None
                } else {
                    self.try_get_imm(r.number(), base_address, data)
                        .map(|v| v.wrapping_add_signed(offset))
                }
            }
            Value::Deref { r, offset } => {
                if r.number() == reg {
                    return None;
                }

                let ptr = self
                    .try_get_imm(r.number(), base_address, data)?
                    .wrapping_add_signed(offset);

                let load = (ptr as usize).checked_sub(base_address)?;
                if data.len() < load + 4 {
                    None
                } else {
                    Some(u32::from_le_bytes(data[load..load + 4].try_into().unwrap()))
                }
            }
        }
    }

    pub fn snapshot(&self) -> [Value; 16] {
        self.regs.clone()
    }

    pub fn merge(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for i in 0..16 {
            let merged = self.regs[i].merge(&other.regs[i]);
            if self.regs[i] != merged {
                self.regs[i] = merged;
                changed = true;
            }
        }
        changed
    }

    pub fn step(&mut self, code: &Code, base_address: usize) {
        self.immediate(15, (base_address + code.pc()) as u32);
        match code.instruction.opcode {
            Opcode::MOV => {
                if let Operand::Reg(r) = code.instruction.operands[0] {
                    if let Operand::Reg(rm) = code.instruction.operands[1] {
                        self.regs[r.number() as usize] = self.regs[rm.number() as usize].clone();
                    } else if let Operand::Imm12(imm) = code.instruction.operands[1] {
                        self.immediate(r.number(), imm as u32);
                    } else if let Operand::Imm32(imm) = code.instruction.operands[1] {
                        self.immediate(r.number(), imm);
                    }
                }
            }
            Opcode::MOVT => {
                if let Operand::Reg(r) = code.instruction.operands[0]
                    && let Operand::Imm32(imm) = code.instruction.operands[1]
                    && let Value::Immediate(existing) = self.get(r.number())
                {
                    self.immediate(r.number(), existing | (imm << 16));
                }
            }
            Opcode::BX | Opcode::BL | Opcode::BLX => {
                self.call();
            }
            Opcode::LDR => {
                if let Operand::Reg(rt) = code.instruction.operands[0]
                    && let Operand::RegDerefPreindexOffset(reg, imm, up, _) =
                        code.instruction.operands[1]
                {
                    let offset = if up { imm as i32 } else { -(imm as i32) };
                    if reg.is_pc() {
                        let abs_pc = (base_address + code.pc()) & !3;
                        let abs_addr = abs_pc.wrapping_add_signed(imm as isize);
                        self.load(rt.number(), abs_addr as u32);
                    } else {
                        self.deref(rt.number(), reg, offset);
                    }
                }
            }
            Opcode::ADD | Opcode::SUB => {
                if let Operand::Reg(rd) = code.instruction.operands[0]
                    && let Operand::Reg(rn) = code.instruction.operands[1]
                    && let Operand::Imm32(imm) = code.instruction.operands[2]
                {
                    let offset = if code.instruction.opcode == Opcode::ADD {
                        imm as i32
                    } else {
                        -(imm as i32)
                    };

                    let new_value = match self.get(rn.number()) {
                        Value::Immediate(val) => Value::Immediate(val.wrapping_add_signed(offset)),
                        Value::Load {
                            base,
                            offset: existing_offset,
                        } => Value::Load {
                            base,
                            offset: existing_offset + offset,
                        },
                        Value::RegisterOffset {
                            r,
                            offset: existing_offset,
                        } => Value::RegisterOffset {
                            r,
                            offset: existing_offset + offset,
                        },
                        _ => Value::RegisterOffset { r: rn, offset },
                    };

                    self.regs[rd.number() as usize] = new_value;
                } else {
                    // Handle Reg + Reg + Reg or mark as Unknown if too complex
                    if let Operand::Reg(rd) = code.instruction.operands[0] {
                        self.regs[rd.number() as usize] = Value::Unknown;
                    }
                }
            }
            _ => (),
        }
    }
}
