use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
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
    reg_analysis::{RegWriteTracker, Value},
    regext::RegExt,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    range: RangeInclusive<usize>,
    code: Vec<Code>,
    snapshot: [Value; 16],
    tail_call: bool,
}

impl BasicBlock {
    pub fn code(&self) -> &[Code] {
        &self.code
    }

    pub fn state_at(&self, target_offset: usize, base_address: usize) -> Option<RegWriteTracker> {
        if !self.range.contains(&target_offset) {
            return None;
        }

        let mut rwt = RegWriteTracker::from_regs(self.snapshot.clone());

        for code in &self.code {
            if code.offset() == target_offset {
                // PC must be updated or bad things gonna happen
                rwt.immediate(15, (base_address + code.pc()) as u32);
                break;
            }

            rwt.step(code, base_address);
        }

        Some(rwt)
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

    /// Analyze function
    pub fn parse(data: &[u8], start: usize, base_address: usize, mode: CpuMode) -> Result<Self> {
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
                println!("{:#x}: {}", block_offset, code.instruction);

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
                            end_of_block = true;
                        } else {
                            leaders.insert(target_offset);
                            if !fn_code.contains_key(&target_offset) {
                                queue.push(target_offset);
                            }

                            // unlike IDA/Ghidra we split this to 2 blocks, not 1
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
                            && reg.is_pc()
                            && rt.is_pc()
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

        // pass 2: split blocks and extract edges for dataflow analysis
        let leader_vec: Vec<usize> = leaders.into_iter().collect();

        struct BlockData {
            start: usize,
            range: RangeInclusive<usize>,
            code: Vec<Code>,
            tail_call: bool,
            successors: Vec<usize>,
        }

        let mut blocks_data: BTreeMap<usize, BlockData> = BTreeMap::new();

        for i in 0..leader_vec.len() {
            let start_addr = leader_vec[i];

            if !fn_code.contains_key(&start_addr) {
                println!("fault at start");
                continue;
            }

            let end_limit = leader_vec.get(i + 1).copied().unwrap_or(usize::MAX);
            let mut block_code = vec![];
            let mut curr_addr = start_addr;
            let mut tail_call = false;
            let mut successors = vec![];

            while curr_addr < end_limit {
                if let Some(code) = fn_code.get(&curr_addr) {
                    block_code.push(code.clone());
                    curr_addr += code.instruction.len().to_const() as usize;

                    let is_last_in_limit = curr_addr >= end_limit;
                    let mut is_end_of_block = false;

                    match code.instruction.opcode {
                        Opcode::B => {
                            is_end_of_block = true;
                            let target_op = code.instruction.operands[0];
                            let target = match target_op {
                                Operand::BranchThumbOffset(target) => target,
                                Operand::BranchOffset(target) => target,
                                _ => unreachable!(),
                            };
                            let target_offset = code.pc().wrapping_add_signed(target as isize);

                            let mut callee_offset = target_offset;
                            let is_tail_call = code.instruction.condition == ConditionCode::AL
                                && (0..5).any(|_| {
                                    Self::disassemble_oneshot(&data[callee_offset..], mode)
                                        .is_ok_and(|c| {
                                            let len = c.instruction.len().to_const() as usize;
                                            callee_offset += len;
                                            c.is_prologue() && c.offset != start
                                        })
                                });

                            if is_tail_call {
                                tail_call = true;
                            } else {
                                successors.push(target_offset);
                                if code.instruction.condition != ConditionCode::AL {
                                    successors.push(curr_addr); // Fallthrough branch condition
                                }
                            }
                        }
                        Opcode::CBZ | Opcode::CBNZ => {
                            is_end_of_block = true;
                            let target_op = code.instruction.operands[1];
                            let target = match target_op {
                                Operand::BranchThumbOffset(target) => target,
                                Operand::BranchOffset(target) => target,
                                _ => unreachable!(),
                            };
                            let target_offset = code.pc().wrapping_add_signed(target as isize);
                            successors.push(target_offset);
                            successors.push(curr_addr); // Fallthrough branch condition
                        }
                        Opcode::POP => {
                            if code.is_epilogue() {
                                is_end_of_block = true;
                            } else if is_last_in_limit {
                                successors.push(curr_addr);
                            }
                        }
                        Opcode::BX | Opcode::ERET => {
                            is_end_of_block = true;
                        }
                        Opcode::LDR => {
                            if let Operand::Reg(rt) = code.instruction.operands[0]
                                && let Operand::RegDerefPreindexOffset(reg, imm, _, _) =
                                    code.instruction.operands[1]
                                && reg.is_pc()
                                && rt.is_pc()
                                && imm == { if mode == CpuMode::Arm { 4 } else { 0 } }
                            {
                                is_end_of_block = true;
                            } else if is_last_in_limit {
                                successors.push(curr_addr);
                            }
                        }
                        _ => {
                            if is_last_in_limit {
                                successors.push(curr_addr);
                            }
                        }
                    }

                    if is_end_of_block {
                        break;
                    }
                } else {
                    break;
                }
            }

            if !block_code.is_empty() {
                let first_offset = block_code.first().unwrap().offset;
                let last_offset = block_code.last().unwrap().offset;

                blocks_data.insert(
                    start_addr,
                    BlockData {
                        start: start_addr,
                        range: first_offset..=last_offset,
                        code: block_code,
                        tail_call,
                        successors,
                    },
                );
            }
        }

        // pass 3: dataflow analysis
        let mut in_states: BTreeMap<usize, Option<RegWriteTracker>> = BTreeMap::new();
        for key in blocks_data.keys() {
            in_states.insert(*key, None);
        }

        let mut list = VecDeque::new();
        in_states.insert(start, Some(RegWriteTracker::new()));
        list.push_back(start);

        while let Some(block_addr) = list.pop_front() {
            if let Some(block) = blocks_data.get(&block_addr) {
                let mut rwt = in_states.get(&block_addr).unwrap().clone().unwrap();

                // we don't know r0-r3 at the start
                if block.start == start {
                    rwt.call();
                }

                for code in &block.code {
                    rwt.step(code, base_address);
                    if block.tail_call && code.instruction.opcode == Opcode::B {
                        rwt.call();
                    }
                }

                for &successor in &block.successors {
                    if let Some(maybe_state) = in_states.get_mut(&successor) {
                        if let Some(state) = maybe_state {
                            if state.merge(&rwt) {
                                if !list.contains(&successor) {
                                    list.push_back(successor);
                                }
                            }
                        } else {
                            *maybe_state = Some(rwt.clone());
                            list.push_back(successor);
                        }
                    }
                }
            }
        }

        let mut blocks = vec![];
        for (_, bdata) in blocks_data {
            let rwt = in_states
                .remove(&bdata.start)
                .flatten()
                .unwrap_or_else(RegWriteTracker::new);
            blocks.push(BasicBlock {
                range: bdata.range,
                code: bdata.code,
                snapshot: rwt.snapshot(),
                tail_call: bdata.tail_call,
            });
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

    pub fn code(&self) -> impl DoubleEndedIterator<Item = &Code> {
        self.blocks.iter().flat_map(|b| b.code())
    }

    pub fn state_at(&self, offset: usize, base_address: usize) -> Option<RegWriteTracker> {
        let block = self.blocks.iter().find(|b| b.range.contains(&offset))?;
        block.state_at(offset, base_address)
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
        functions.push(Function::parse(data, 0, base_address, entry_mode)?);

        loop {
            if let Some(i) = queue.pop() {
                let function = functions[i].clone();
                let mode = function.mode;

                for block in function.blocks {
                    for (j, code) in block.code.iter().enumerate() {
                        match code.instruction.opcode {
                            Opcode::BX => {
                                if let Operand::Reg(r) = code.instruction.operands[0]
                                    && !r.is_lr()
                                {
                                    let state = block
                                        .state_at(code.offset(), base_address)
                                        .ok_or(Error::NoState(code.offset))?;
                                    if let Some(mut jump_addr) =
                                        state.try_get_imm(r.number(), base_address, data)
                                    {
                                        if jump_addr as usize >= base_address {
                                            jump_addr -= base_address as u32;
                                        } else {
                                            println!(
                                                "we need better register tracking, skip this bx @ {} at {:#x}, resolved jump addr = {:#x}",
                                                code.instruction, code.offset, jump_addr
                                            );
                                            println!("regs:");
                                            for reg in 0..16 {
                                                println!("{reg}: {:?}", state.get(reg));
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

                                        functions.push(Function::parse(
                                            data,
                                            jump_addr,
                                            base_address,
                                            mode,
                                        )?);
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
                                            println!("{reg}: {:?}", state.get(reg));
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

                                    functions.push(Function::parse(
                                        data,
                                        addr,
                                        base_address,
                                        mode,
                                    )?);
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
                                    functions.push(Function::parse(
                                        data,
                                        addr,
                                        base_address,
                                        mode,
                                    )?);

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
