use derive_ctor::ctor;
use memchr::memmem;
use yaxpeax_arch::LengthedInstruction;

use crate::{
    disasm::{disassemble_arm, disassemble_thumb},
    err::Error,
};
use yaxpeax_arm::armv7::{ConditionCode, Instruction, Opcode, Operand};

mod disasm;
pub mod err;

pub type Result<T> = core::result::Result<T, Error>;

pub use yaxpeax_arm;

#[derive(Debug, ctor)]
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
}

pub struct Analyzer<'a> {
    data: &'a [u8],
    code: Vec<Code>,
}

impl<'a> Analyzer<'a> {
    pub fn new_thumb(data: &'a [u8]) -> Self {
        Self {
            data,
            code: disassemble_thumb(data),
        }
    }

    pub fn new_arm(data: &'a [u8]) -> Self {
        Self {
            data,
            code: disassemble_arm(data),
        }
    }

    /// Map instruction offset to the index
    #[inline(always)]
    fn offset2idx(&self, offset: usize) -> Option<usize> {
        self.code
            .iter()
            .enumerate()
            .find(|(_, inst)| inst.offset == offset)
            .map(|(i, _)| i)
    }

    /// Does RegList have LR?
    #[inline(always)]
    fn list_has_lr(list: u16) -> bool {
        list & (1 << 14) != 0
    }

    /// Does RegList have PC?
    #[inline(always)]
    fn list_has_pc(list: u16) -> bool {
        list & (1 << 15) != 0
    }

    /// Get index of the instruction containing reference to the string
    ///
    /// # Errors
    /// [Error::NotFound] if the imm12 range is exhausted
    pub fn find_string_ref(&self, s: &str) -> Result<usize> {
        const IMM12_MAX: usize = 0x7ff; // signed

        let string_offset = memmem::find_iter(self.data, s.as_bytes())
            .next()
            .ok_or(Error::NotFound)?;

        let range = string_offset - IMM12_MAX..string_offset + IMM12_MAX;
        for (i, code) in self.code.iter().enumerate() {
            if !range.contains(&code.offset) {
                continue;
            }

            match code.instruction.opcode {
                Opcode::ADR => match code.instruction.operands[1] {
                    Operand::Imm32(imm) => {
                        let load = if code.offset > string_offset {
                            code.offset - string_offset
                        } else {
                            string_offset - code.offset
                        } - 2;

                        if imm == load as u32 {
                            return Ok(i);
                        }
                    }
                    _ => unreachable!("unexpected operand"),
                },
                _ => (),
            }
        }

        Err(Error::NotFound)
    }

    /// Get iterator of instructions for the guessed function from `i` index in [start..=end] range.
    ///
    /// # Errors
    /// [Error::NotFound] if the offset mapping failed. It shouldn't be raised unless there's a bug
    pub fn find_function_bounds(&self, i: usize) -> Result<&[Code]> {
        let mut start = 0;
        let mut end = 0;

        for code in self.code[..i].iter().rev() {
            // First PUSH opcode with LR is likely function start
            if code.instruction.opcode == Opcode::PUSH {
                if let Operand::RegList(list) = code.instruction.operands[0]
                    && Self::list_has_lr(list)
                {
                    start = self.offset2idx(code.offset).ok_or(Error::NotFound)?;
                    break;
                }
            }
        }

        // Now carefully walk until we find the very end of the function
        //
        // XXX: this is dumb decoder, we need a tail calls and simple flow-based matching here...
        for code in self.code[start..].iter() {
            match code.instruction.opcode {
                // POP {..., LR} or POP {..., PC}
                //
                // XXX: POP LR is not function end
                Opcode::POP => {
                    if let Operand::RegList(list) = code.instruction.operands[0]
                        && (Self::list_has_lr(list) || Self::list_has_pc(list))
                    {
                        end = self.offset2idx(code.offset).ok_or(Error::NotFound)?;
                        break;
                    }
                }

                // BX LR
                Opcode::BX => {
                    if let Operand::Reg(r) = code.instruction.operands[0]
                        && r.number() == 14
                    {
                        end = self.offset2idx(code.offset).ok_or(Error::NotFound)?;
                        break;
                    }
                }
                _ => (),
            }
        }

        self.code.get(start..=end).ok_or(Error::NotFound)
    }

    /// Get basic blocks in the given `range`
    ///
    /// # Errors
    /// [Error::NotFound] if the offset mapping failed. It shouldn't be raised unless there's a bug
    pub fn find_basic_blocks(
        &self,
        range: impl IntoIterator<Item = &'a Code>,
    ) -> Result<Vec<&[Code]>> {
        let mut starts = Vec::with_capacity(10);

        for (i, code) in range.into_iter().enumerate() {
            if i == 0 {
                // Entry is always a block
                starts.push(self.offset2idx(code.offset).ok_or(Error::NotFound)?);
            }

            match code.instruction.opcode {
                Opcode::B => {
                    if let Operand::BranchThumbOffset(target) = code.instruction.operands[0] {
                        // XXX: unconditional jumps use + 4 for PC value as per ARM spec,
                        // but conditional use + 2 due to +1 in the yaxpeax code, which
                        // becomes 2 after shifting. See https://github.com/iximeow/yaxpeax-arm/blob/5803a74b89cfc986f26b01f607bcfedd7bcbcf68/src/armv7/thumb.rs#L4186
                        //
                        // XXX: report this bug to the upstream yaxpeax...
                        let fixup = if code.instruction.condition == ConditionCode::AL {
                            4
                        } else {
                            2
                        };
                        let pc = code.offset + fixup;
                        let off = target << 1;

                        let target = pc.checked_add_signed(off as isize).unwrap();
                        starts.push(self.offset2idx(target).ok_or(Error::NotFound)?);
                    }
                }
                Opcode::POP => {
                    if let Operand::RegList(list) = code.instruction.operands[0]
                        && Self::list_has_pc(list)
                    {
                        starts.push(
                            self.offset2idx(
                                code.offset + code.instruction.len().to_const() as usize,
                            )
                            .ok_or(Error::NotFound)?,
                        );
                    }
                }
                _ => (),
            }
        }

        starts.sort_unstable();
        Ok(starts
            .windows(2)
            .map(|w| (w[0], w[1]))
            .map(|(curr, next)| &self.code[curr..next])
            .collect())
    }
}
