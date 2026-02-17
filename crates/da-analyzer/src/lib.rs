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

/// IR struct for basic block detection
struct BasicBlock {
    start: usize,
    end: usize,
}

impl BasicBlock {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

impl PartialEq for BasicBlock {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start
    }
}

impl Eq for BasicBlock {}

impl PartialOrd for BasicBlock {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.start > other.start {
            Some(std::cmp::Ordering::Greater)
        } else if self.start == other.start {
            Some(std::cmp::Ordering::Equal)
        } else {
            Some(std::cmp::Ordering::Less)
        }
    }
}

impl Ord for BasicBlock {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.start > other.start {
            std::cmp::Ordering::Greater
        } else if self.start == other.start {
            std::cmp::Ordering::Equal
        } else {
            std::cmp::Ordering::Less
        }
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

    /// Does this instruction look like function start?
    #[inline(always)]
    fn is_prologue(code: &Code) -> bool {
        if let Operand::RegList(list) = code.instruction.operands[0]
            && code.instruction.opcode == Opcode::PUSH
            && Self::list_has_lr(list)
        {
            true
        } else {
            false
        }
    }

    /// Does this instruction look like function end?
    #[inline(always)]
    fn is_epilogue(code: &Code) -> bool {
        if let Operand::RegList(list) = code.instruction.operands[0]
            && code.instruction.opcode == Opcode::POP
            && Self::list_has_pc(list)
        {
            true
        } else {
            false
        }
    }

    /// Get index of the instruction containing reference to the string
    ///
    /// # Errors
    /// [Error::MapOffsetToIndex] if the imm12 range is exhausted
    pub fn find_string_ref(&self, s: &str) -> Result<usize> {
        const IMM12_MAX: usize = 0x7ff; // signed

        let string_offset = memmem::find_iter(self.data, s.as_bytes())
            .next()
            .ok_or(Error::MapOffsetToIndex)?;

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

        Err(Error::MapOffsetToIndex)
    }

    /// Find all basic blocks beloging to the function at the `i` index
    ///
    /// # Errors
    /// Generally they shouldn't happen unless there's a bug in the analyzer
    /// - [Error::MapOffsetToIndex] if offset mapping failed
    /// - [Error::InvalidBlockIndex] queue and actual blocks lengths don't match
    /// - [Error::Overrun] analyzer got out of bounds of the current function due to block split failure
    /// - [Error::PCOverflow] PC fixup failed
    pub fn analyze_function(&self, i: usize) -> Result<Vec<&[Code]>> {
        let mut start = 0;

        for code in self.code[..i].iter().rev() {
            if Self::is_prologue(code) {
                start = self
                    .offset2idx(code.offset)
                    .ok_or(Error::MapOffsetToIndex)?;
                break;
            }
        }

        let mut queue = vec![start];
        let mut blocks = vec![BasicBlock::new(start, self.code.len())];

        while let Some(code_start) = queue.pop() {
            let block_idx = match blocks.iter().position(|b| b.start == code_start) {
                Some(v) => v,
                None => return Err(Error::InvalidBlockIndex),
            };

            for code in self.code[code_start..].iter() {
                let idx = self
                    .offset2idx(code.offset)
                    .ok_or(Error::MapOffsetToIndex)?;

                if idx > code_start && blocks.iter().any(|b| b.start == idx) {
                    // truncate block
                    blocks[block_idx].end = idx - 1;
                    break;
                }

                if idx > start && Self::is_prologue(code) {
                    return Err(Error::Overrun);
                }

                match code.instruction.opcode {
                    Opcode::B => {
                        if let Operand::BranchThumbOffset(target) = code.instruction.operands[0] {
                            // XXX: unconditional jumps use + 4 for PC value as per ARM spec,
                            // but conditional use + 2 due to +1 in the yaxpeax code, which
                            // becomes 2 after shifting. See https://github.com/iximeow/yaxpeax-arm/blob/5803a74b89cfc986f26b01f607bcfedd7bcbcf68/src/armv7/thumb.rs#L4186
                            let fixup = if code.instruction.condition == ConditionCode::AL {
                                4
                            } else {
                                code.instruction.len().to_const() as usize
                            };
                            let pc = code.offset + fixup;
                            let off = target << 1;

                            let target = self
                                .offset2idx(
                                    pc.checked_add_signed(off as isize)
                                        .ok_or(Error::PCOverflow)?,
                                )
                                .ok_or(Error::MapOffsetToIndex)?;

                            let block_start = blocks[block_idx].start;
                            // blocks can't have the same end, fixup the previous one to point to the correct end
                            if let Some(fixup_block) = blocks.iter_mut().find(|b| b.end == idx) {
                                fixup_block.end = block_start;
                            }

                            // current block ends where first branch starts
                            blocks[block_idx].end = idx;

                            // let's see what the target says
                            //
                            // 5 is usually enough for stack frame fixup and other stuff function might do
                            // if it's external call, then it should have PUSH {LR}
                            //
                            // internal branches never (well, i've never seen that) have PUSH {LR}
                            //
                            // XXX: walk until first B is found, to ensure the PUSH is not somewhere deeper
                            let is_tail_call = code.instruction.condition == ConditionCode::AL
                                && self.code[target..target + 5]
                                    .iter()
                                    .any(|code| Self::is_prologue(code));
                            if is_tail_call {
                                break;
                            }

                            // target is already existing block? skip
                            if !blocks.iter().any(|b| b.start == target) {
                                queue.push(target);
                                blocks.push(BasicBlock::new(target, self.code.len()));

                                if code.instruction.condition != ConditionCode::AL {
                                    queue.push(idx + 1);
                                    blocks.push(BasicBlock::new(idx + 1, self.code.len()));
                                }
                            }

                            // the block is already ended, remember?
                            break;
                        }
                    }

                    Opcode::POP => {
                        if Self::is_epilogue(code) {
                            blocks[block_idx].end = idx;
                            break;
                        }
                    }

                    Opcode::BX => {
                        if let Operand::Reg(r) = code.instruction.operands[0]
                            && r.number() == 14
                        {
                            blocks[block_idx].end = idx;
                            break;
                        }
                    }

                    _ => (),
                }
            }
        }

        blocks.sort_unstable();

        // check if all blocks have valid end address
        assert!(!blocks.iter().any(|b| b.end == self.code.len()));

        assert!(blocks.len() > 0);
        assert_eq!(queue.len(), 0);

        Ok(blocks
            .into_iter()
            .map(|b| &self.code[b.start..=b.end])
            .collect())
    }
}
