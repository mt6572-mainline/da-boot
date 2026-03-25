use derive_ctor::ctor;
use memchr::memmem;

use crate::{
    cpu_mode::CpuMode,
    err::Error,
    fn_analysis::{Function, FunctionAnalysis},
    regext::{RegExt, RegListExt},
};
use yaxpeax_arm::armv7::{Instruction, Opcode, Operand};

pub mod cpu_mode;
mod disasm;
pub mod err;
pub mod fn_analysis;
pub(crate) mod reg_analysis;
pub(crate) mod regext;

pub type Result<T> = core::result::Result<T, Error>;

pub use yaxpeax_arm;

#[derive(Debug, Default, Clone, PartialEq, Eq, ctor)]
pub struct Code {
    instruction: Instruction,
    offset: usize,
}

impl Code {
    #[inline(always)]
    pub fn instruction(&self) -> &Instruction {
        &self.instruction
    }

    #[inline(always)]
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Does this instruction look like function start?
    #[inline(always)]
    pub(crate) fn is_prologue(&self) -> bool {
        let is_push = self.instruction.opcode == Opcode::PUSH;
        if let Operand::RegList(list) = self.instruction.operands[0] {
            is_push && list.has_lr()
        } else if let Operand::Reg(r) = self.instruction.operands[0] {
            is_push && r.is_lr()
        } else {
            false
        }
    }

    /// Does this instruction look like function end?
    #[inline(always)]
    pub(crate) fn is_epilogue(&self) -> bool {
        let is_pop = self.instruction.opcode == Opcode::POP;
        if let Operand::RegList(list) = self.instruction.operands[0] {
            is_pop && list.has_pc()
        } else if let Operand::Reg(r) = self.instruction.operands[0] {
            is_pop && r.is_pc()
        } else {
            false
        }
    }

    /// Get PC offset for current instruction
    pub(crate) fn pc(&self) -> usize {
        self.offset + if self.instruction.thumb() { 4 } else { 8 }
    }
}

pub struct Analyzer {
    data: Vec<u8>,
    f: FunctionAnalysis,
}

impl Analyzer {
    pub fn try_new(data: Vec<u8>, base_address: usize, entry_mode: CpuMode) -> Result<Self> {
        let f = FunctionAnalysis::analyze_from_entrypoint(&data, base_address, entry_mode)?;
        Ok(Self { data, f })
    }

    pub fn find_string_ref(&self, s: &str) -> Option<(&Function, usize)> {
        let off = memmem::find_iter(&self.data, s.as_bytes()).next()?;

        self.f.fns.iter().find_map(|f| {
            f.blocks()
                .iter()
                .enumerate()
                .find_map(|(block_idx, b)| {
                    let has_match = b.code().iter().enumerate().any(|(code_rel_off, code)| {
                        match code.instruction.opcode {
                            Opcode::ADR => {
                                if let Operand::Imm32(imm) = code.instruction.operands[1] {
                                    let load = if code.offset > off {
                                        code.offset - off
                                    } else {
                                        off - code.offset
                                    } as u32
                                        - 2;

                                    imm == load || imm == load - 2
                                } else {
                                    false
                                }
                            }
                            Opcode::LDR => {
                                if let Operand::Reg(r) = code.instruction.operands[0]
                                    && let Operand::RegDerefPreindexOffset(
                                        r_ldr_should_be_pc,
                                        imm,
                                        _,
                                        _,
                                    ) = code.instruction.operands[1]
                                    && r_ldr_should_be_pc.is_pc()
                                {
                                    let load = (code.pc() & !3) + imm as usize;
                                    let pool = u32::from_le_bytes(
                                        self.data[load..load + 4].try_into().unwrap(),
                                    ) as usize;

                                    b.code()[code_rel_off..].iter().any(|add| {
                                        if let Operand::Reg(rt) = add.instruction.operands[0]
                                            && let Operand::Reg(r_should_be_pc) =
                                                add.instruction.operands[1]
                                        {
                                            add.instruction.opcode == Opcode::ADD
                                                && r_should_be_pc.is_pc()
                                                && rt == r
                                                && (add.pc() + pool) & !3 == off & !3 // XXX: uh oh
                                        } else {
                                            false
                                        }
                                    })
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        }
                    });

                    if has_match { Some(block_idx) } else { None }
                })
                .map(|block_idx| (f, block_idx))
        })
    }
}
