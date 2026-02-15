use yaxpeax_arch::{Arch, Decoder, Reader, ReaderBuilder, U8Reader};
use yaxpeax_arm::armv7::{ARMv7, DecodeError, InstDecoder};

use crate::Code;

fn disassemble(decoder: InstDecoder, data: &[u8]) -> Vec<Code> {
    let mut reader =
        ReaderBuilder::<<ARMv7 as Arch>::Address, <ARMv7 as Arch>::Word>::read_from(data);

    let mut vec = Vec::with_capacity(10 * 1024);

    loop {
        let address =
            <U8Reader<'_> as Reader<u32, <ARMv7 as Arch>::Word>>::total_offset(&mut reader);
        let decode_res = decoder.decode(&mut reader);

        match decode_res {
            Ok(inst) => {
                vec.push(Code::new(inst, address as usize));
            }
            Err(e) => match e {
                DecodeError::ExhaustedInput => break,
                _ => (), // Decode errors are not fatal, we don't know if it's data disassembled as junk or incomplete decoder
            },
        }
    }

    vec
}

pub fn disassemble_thumb(data: &[u8]) -> Vec<Code> {
    disassemble(InstDecoder::armv7_thumb(), data)
}

pub fn disassemble_arm(data: &[u8]) -> Vec<Code> {
    disassemble(InstDecoder::armv7(), data)
}
