use std::{ops::RangeInclusive, sync::LazyLock};

use regex::Regex;

use crate::{Disassembler, Result, err::Error};

/// Fuzzy search regex to parse registers from capstone output
///
/// - Supports regular values like `r0`, `r1`, up to `r15`, `sb`, `sp`, `lr`, `pc`
/// - Register dereference (values in brackets like `[r0]`)
/// - Inline valies (`#0x0`, `#1`)
/// - Fuzzy register as `r?` to match any register or value (supported for both regular form like `r?` and dereference like `[r?]`)
static FUZZY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:\[\s*(?:r(?:1?[0-5]|[0-9]|\?)|sb|sp|lr|pc)\s*\]|(?:r(?:1?[0-5]|[0-9]|\?)|sb|sp|lr|pc)|#(?:0x[0-9a-fA-F]+|\d+)|#\?|\?)",
    )
    .unwrap()
});

#[inline]
fn is_special_reg(reg: &str) -> bool {
    reg == "sb" || reg == "sp" || reg == "lr" || reg == "pc" || reg == "fp"
}

pub fn generic_reg_matcher(m: &str, op: &str, want: &str) -> Result<bool> {
    if want == "??" {
        return Ok(true);
    }

    let (want_m, want_op) = want.split_once(' ').ok_or(Error::PatternNotFound)?;

    // `??` for entire match or for operand means anything,
    // No need to use regex for equal instructions,
    // Neither for same operands but any mnemonic
    if (m == want_m && op == want_op)
        || (m != want_m && op == want_op && want_m == "?")
        || (m == want_m && want_op == "??")
    {
        Ok(true)
    } else if want_op.contains('?') && (want_m == "?" || m == want_m) {
        let has_regs = FUZZY_REGEX.find_iter(op).collect::<Vec<_>>();
        let want_regs = FUZZY_REGEX.find_iter(want_op).collect::<Vec<_>>();

        if has_regs.len() == want_regs.len() {
            Ok(has_regs
                .into_iter()
                .zip(want_regs)
                .map(|(hr, wr)| (hr.as_str(), wr.as_str()))
                .all(|(hr, wr)| {
                    wr == "?"
                        || (wr == hr)
                        || (wr == "r?" && (hr.starts_with('r') || is_special_reg(hr)))
                        || (wr == "#?" && hr.starts_with('#'))
                }))
        } else {
            Ok(false)
        }
    } else {
        Ok(false)
    }
}

pub fn fuzzy_search_thumb2<T: Fn(&str, &str, &str) -> Result<bool>>(
    disasm: &Disassembler,
    slice: &[u8],
    pattern: &str,
    matcher: T,
) -> Result<RangeInclusive<usize>> {
    let mut offset = 0usize;

    let mut n = 0;
    let mut start = None;
    let split_instr = pattern
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    while offset < slice.len() {
        let insns = disasm.thumb2_disasm_count(&slice[offset..], 1)?;

        if let Some(insn) = insns.iter().next() {
            let size = insn.bytes().len();

            let m = insn.mnemonic().ok_or(Error::MnemonicNotAvailable)?;
            let op = insn.op_str().ok_or(Error::InstrOpNotAvailable)?;
            let want = split_instr.get(n).ok_or(Error::PatternNotFound)?;

            if matcher(m, op, want)? {
                if n == 0 {
                    start = Some(offset);
                }

                n += 1;

                if n == split_instr.len() {
                    return Ok(start.ok_or(Error::PatternNotFound)?..=offset + size);
                }
            } else if n != 0 {
                n = 0;
                start = None;
            }

            offset += size;
        } else {
            // thumb2 align
            offset += 2;
        }
    }

    Err(Error::PatternNotFound)
}
