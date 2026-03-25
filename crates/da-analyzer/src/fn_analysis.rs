use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    ops::RangeInclusive,
};

use yaxpeax_arch::LengthedInstruction;
use yaxpeax_arm::armv7::{ConditionCode, Opcode, Operand};

use crate::{
    Code, Result,
    cpu_mode::CpuMode,
    disasm::{disassemble_arm_oneshot, disassemble_thumb_oneshot},
    err::Error,
    reg_analysis::RegWriteTracker,
    regext::RegExt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    range: RangeInclusive<usize>,
    code: Vec<Code>,
    tail_call: bool,
}

impl BasicBlock {
    pub fn code(&self) -> &[Code] {
        &self.code
    }
}

impl Display for BasicBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for code in &self.code {
            write!(f, "{:#x}: {}", code.offset, code.instruction)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    entry: usize,
    mode: CpuMode,
    blocks: Vec<BasicBlock>,
}

impl Display for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Function in {:?} mode", self.mode)?;
        for (i, block) in self.blocks.iter().enumerate() {
            writeln!(f, "block {i}:")?;
            write!(f, "{block}")?;
        }

        Ok(())
    }
}

impl Function {
    fn disassemble_oneshot(data: &[u8], mode: CpuMode) -> Result<Code> {
        match mode {
            CpuMode::Arm => match disassemble_arm_oneshot(data) {
                Ok(v) => Ok(v),
                Err(Error::Disassembler(yaxpeax_arm::armv7::DecodeError::Incomplete)) => Ok(Code {
                    instruction: yaxpeax_arm::armv7::Instruction {
                        condition: ConditionCode::AL,
                        opcode: Opcode::NOP,
                        operands: [
                            Operand::Nothing,
                            Operand::Nothing,
                            Operand::Nothing,
                            Operand::Nothing,
                        ],
                        s: false,
                        wide: false,
                        thumb_w: false,
                        thumb: false,
                    },
                    offset: 0,
                }),
                Err(e) => Err(e),
            },
            CpuMode::Thumb => disassemble_thumb_oneshot(data),
        }
    }

    /// - data = global binary data
    /// - start = offset to the first instruction of this function
    /// - mode = arm/thumb mode, caller must determine it by BL/BLX and previous mode state
    pub fn parse(data: &[u8], start: usize, mode: CpuMode) -> Result<Self> {
        println!("start analysis for {:#x}", start);
        let mut fn_code: BTreeMap<usize, Code> = BTreeMap::new();
        let mut leaders: BTreeSet<usize> = BTreeSet::new();

        let mut queue = vec![start];
        leaders.insert(start);

        let mut prev_opcode: Option<Opcode>;

        println!("do pass 1");
        // pass 1: control flow discovery
        while let Some(mut block_offset) = queue.pop() {
            prev_opcode = None;

            println!("analyze block at {:#x}", block_offset);
            loop {
                if fn_code.contains_key(&block_offset) {
                    leaders.insert(block_offset);
                    break;
                }

                let mut code = match Self::disassemble_oneshot(&data[block_offset..], mode) {
                    Ok(c) => c,
                    Err(Error::Disassembler(yaxpeax_arm::armv7::DecodeError::Incomplete)) => {
                        println!(
                            "disassembler incomplete at instruction after {:#x}",
                            block_offset
                        );
                        break;
                    }
                    Err(e) => Err(e)?,
                };
                //println!("{:#x}: {}", block_offset, code.instruction);

                code.offset = block_offset;
                let insn_len = code.instruction.len().to_const() as usize;
                let next_offset = block_offset + insn_len;

                if code.offset != start && code.is_prologue() && prev_opcode != Some(Opcode::PUSH) {
                    println!(
                        "Fell through into new function prologue at {:#x}, truncating.",
                        code.offset
                    );
                    break;
                }

                prev_opcode = Some(code.instruction.opcode);

                fn_code.insert(block_offset, code.clone());

                let mut end_of_block = false;

                match code.instruction.opcode {
                    Opcode::B | Opcode::CBZ | Opcode::CBNZ => {
                        let is_cbz_cbnz =
                            matches!(code.instruction.opcode, Opcode::CBZ | Opcode::CBNZ);
                        let target_op = if code.instruction.opcode == Opcode::B {
                            code.instruction.operands[0]
                        } else {
                            code.instruction.operands[1]
                        };

                        let target = match target_op {
                            Operand::BranchThumbOffset(target) => target,
                            Operand::BranchOffset(target) => target,
                            _ => unreachable!(),
                        };

                        let target_offset = code.pc().wrapping_add_signed(target as isize);

                        let is_tail_call = code.instruction.condition == ConditionCode::AL && {
                            let mut callee_offset = target_offset;
                            (0..5)
                                .map(|_| {
                                    Self::disassemble_oneshot(&data[callee_offset..], mode).map(
                                        |mut callee_code| {
                                            callee_code.offset = callee_offset;
                                            callee_offset +=
                                                callee_code.instruction.len().to_const() as usize;
                                            callee_code
                                        },
                                    )
                                })
                                .any(|callee_code| {
                                    callee_code.is_ok_and(|callee_code| {
                                        callee_code.is_prologue() && callee_code.offset != start
                                    })
                                })
                        };

                        if is_tail_call {
                            // Tail calls leave the function, do not queue the target.
                            end_of_block = true;
                        } else {
                            // Target is a leader
                            leaders.insert(target_offset);
                            if !fn_code.contains_key(&target_offset) {
                                queue.push(target_offset);
                            }

                            // If conditional, the fallthrough is also a leader
                            if code.instruction.condition != ConditionCode::AL || is_cbz_cbnz {
                                leaders.insert(next_offset);
                                if !fn_code.contains_key(&next_offset) {
                                    queue.push(next_offset);
                                }
                            }
                            end_of_block = true;
                        }
                    }

                    Opcode::POP => {
                        if code.is_epilogue() {
                            end_of_block = true;
                        }
                    }

                    Opcode::BX | Opcode::ERET => {
                        end_of_block = true;
                    }

                    Opcode::LDR => {
                        if let Operand::Reg(rt) = code.instruction.operands[0]
                            && let Operand::RegDerefPreindexOffset(reg, imm, _, _) =
                                code.instruction.operands[1]
                            && rt.is_pc()
                            && reg.is_pc()
                            && imm == { if mode == CpuMode::Arm { 4 } else { 0 } }
                        {
                            end_of_block = true;
                        }
                    }

                    _ => (),
                }

                if end_of_block {
                    break;
                } else {
                    block_offset = next_offset;
                }
            }
        }
        println!("done, do pass 2");

        // pass 2: split blocks
        let leader_vec: Vec<usize> = leaders.into_iter().collect();
        let mut blocks = Vec::new();

        for i in 0..leader_vec.len() {
            let start_addr = leader_vec[i];

            if !fn_code.contains_key(&start_addr) {
                println!("fault at start");
                continue;
            }

            let end_limit = leader_vec.get(i + 1).copied().unwrap_or(usize::MAX);
            let mut block_code = Vec::new();
            let mut curr_addr = start_addr;
            let mut tail_call = false;

            while curr_addr < end_limit {
                if let Some(code) = fn_code.get(&curr_addr) {
                    block_code.push(code.clone());
                    curr_addr += code.instruction.len().to_const() as usize;

                    if code.instruction.opcode == Opcode::B
                        && code.instruction.condition == ConditionCode::AL
                    {
                        let target_op = code.instruction.operands[0];
                        let target = match target_op {
                            Operand::BranchThumbOffset(target) => target,
                            Operand::BranchOffset(target) => target,
                            _ => unreachable!(),
                        };
                        let target_offset = code.pc().wrapping_add_signed(target as isize);
                        let mut callee_offset = target_offset;
                        let is_tail_call = (0..5).any(|_| {
                            Self::disassemble_oneshot(&data[callee_offset..], mode).is_ok_and(|c| {
                                let len = c.instruction.len().to_const() as usize;
                                callee_offset += len;
                                c.is_prologue() && c.offset != start
                            })
                        });

                        if is_tail_call {
                            tail_call = true;
                        }
                    }
                } else {
                    break;
                }
            }

            if !block_code.is_empty() {
                let first_offset = block_code.first().unwrap().offset;
                let last_offset = block_code.last().unwrap().offset;

                blocks.push(BasicBlock {
                    range: first_offset..=last_offset,
                    code: block_code,
                    tail_call,
                });
            }
        }
        println!("done");

        Ok(Self {
            entry: start,
            mode,
            blocks,
        })
    }

    pub fn blocks(&self) -> &[BasicBlock] {
        &self.blocks
    }

    pub fn mode(&self) -> CpuMode {
        self.mode
    }

    pub fn entry(&self) -> usize {
        self.entry
    }
}

pub struct FunctionAnalysis {
    pub fns: Vec<Function>,
}

impl FunctionAnalysis {
    pub fn analyze_from_entrypoint(
        data: &[u8],
        base_address: usize,
        entry_mode: CpuMode,
    ) -> Result<Self> {
        let mut queue = Vec::with_capacity(100);
        let mut functions = Vec::with_capacity(100);

        queue.push(0);
        functions.push(Function::parse(data, 0, entry_mode)?);

        loop {
            if let Some(i) = queue.pop() {
                let function = functions[i].clone();
                let mode = function.mode;

                let mut regs = RegWriteTracker::new();
                for block in function.blocks {
                    for (j, code) in block.code.iter().enumerate() {
                        regs.store(15, (base_address + code.pc()) as u32);
                        match code.instruction.opcode {
                            Opcode::MOV => {
                                if let Operand::Reg(r) = code.instruction.operands[0]
                                    && let Operand::Imm32(imm) = code.instruction.operands[1]
                                {
                                    regs.store(r.number(), imm);
                                }
                            }
                            Opcode::MOVT => {
                                if let Operand::Reg(r) = code.instruction.operands[0]
                                    && let Operand::Imm32(imm) = code.instruction.operands[1]
                                {
                                    regs.store(
                                        r.number(),
                                        regs.get(r.number()).unwrap_or(0) | imm >> 16,
                                    );
                                }
                            }

                            Opcode::BX => {
                                if let Operand::Reg(r) = code.instruction.operands[0]
                                    && !r.is_lr()
                                {
                                    if let Some(mut jump_addr) = regs.get(r.number()) {
                                        if jump_addr as usize >= base_address {
                                            jump_addr -= base_address as u32;
                                        } else {
                                            println!(
                                                "we need better register tracking, skip this bx @ {} at {:#x}, resolved jump addr = {:#x}",
                                                code.instruction, code.offset, jump_addr
                                            );
                                            println!("regs:");
                                            for reg in 0..16 {
                                                println!("{reg}: {:?}", regs.get(reg));
                                            }

                                            continue;
                                        }

                                        let mode = if jump_addr & 1 != 0 {
                                            CpuMode::Thumb
                                        } else {
                                            CpuMode::Arm
                                        };
                                        let jump_addr = (jump_addr & !1) as usize;

                                        if jump_addr >= data.len() {
                                            println!(
                                                "looks like jumpout or register tracking issue... @ {:#x}: {} (jump addr = {:#x})",
                                                code.offset, code.instruction, jump_addr
                                            );
                                            continue;
                                        }

                                        if functions.iter().find(|f| f.entry == jump_addr).is_some()
                                        {
                                            continue;
                                        }

                                        functions.push(Function::parse(data, jump_addr, mode)?);
                                        queue.push(functions.len() - 1);
                                    } else {
                                        println!("{} reg is failed to track!", code.instruction);
                                        println!("code:");
                                        for code in &block.code {
                                            println!(
                                                "{:#x}: {} ({:?})",
                                                code.offset,
                                                code.instruction,
                                                code.instruction.opcode
                                            );
                                        }
                                        println!("regs:");
                                        for reg in 0..16 {
                                            println!("{reg}: {:?}", regs.get(reg));
                                        }
                                    }
                                }
                            }

                            Opcode::B | Opcode::BL | Opcode::BLX => {
                                if let Operand::BranchOffset(imm)
                                | Operand::BranchThumbOffset(imm) = code.instruction.operands[0]
                                {
                                    if code.instruction.opcode == Opcode::B
                                        && !(block.tail_call && j == block.code.len() - 1)
                                    {
                                        continue;
                                    }

                                    let mut pc = code.pc();
                                    if mode == CpuMode::Thumb
                                        && code.instruction.opcode == Opcode::BLX
                                    {
                                        pc &= !3;
                                    }

                                    let mode = if code.instruction.opcode == Opcode::BLX {
                                        !mode
                                    } else {
                                        mode
                                    };

                                    let addr = pc.wrapping_add_signed(imm as isize);

                                    if functions.iter().find(|f| f.entry == addr).is_some() {
                                        continue;
                                    }

                                    functions.push(Function::parse(data, addr, mode)?);
                                    queue.push(functions.len() - 1);
                                }
                            }

                            Opcode::LDR => {
                                if let Operand::Reg(rt) = code.instruction.operands[0]
                                    && let Operand::RegDerefPreindexOffset(reg, imm, up, _) =
                                        code.instruction.operands[1]
                                    // XXX: no.
                                    && reg.is_pc()
                                {
                                    let start = if up {
                                        code.pc() + imm as usize
                                    } else {
                                        code.pc() - imm as usize
                                    };

                                    let addr = u32::from_le_bytes(
                                        data[start..start + 4].try_into().unwrap(),
                                    );

                                    // XXX: do we want rel or abs addr?
                                    regs.store(rt.number(), addr);

                                    if !(rt.is_pc()
                                        && imm == { if mode == CpuMode::Arm { 4 } else { 0 } })
                                    {
                                        continue;
                                    }

                                    let mode = if addr & 1 != 0 {
                                        CpuMode::Thumb
                                    } else {
                                        CpuMode::Arm
                                    };

                                    let addr = addr as usize;
                                    if addr < base_address || addr - base_address >= data.len() {
                                        continue;
                                    }

                                    let addr = (addr - base_address) & !1;
                                    if functions.iter().find(|f| f.entry == addr).is_some() {
                                        continue;
                                    }

                                    println!("do");
                                    functions.push(Function::parse(data, addr, mode)?);

                                    queue.push(functions.len() - 1);
                                }
                            }

                            _ => (),
                        }
                    }
                }
            } else {
                break;
            }
        }

        Ok(Self { fns: functions })
    }
}
