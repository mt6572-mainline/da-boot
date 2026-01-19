use std::{borrow::Cow, fs};

use colored::Colorize;
use da_parser::parse_da;
use da_patcher::{Assembler, Disassembler, Patch, PatchCollection};
use da_soc::SoC;
use simpleport::{Port, SimpleRead, SimpleWrite};

use crate::{
    CommandDA, Result,
    commands::{
        da::DA1Setup,
        preloader::{JumpDA, SendDA},
    },
    err::Error,
    exploit::{Exploit, Exploits},
    log, status,
};

pub fn run_da1(soc: SoC, mut port: Port, command: CommandDA) -> Result<()> {
    let mut file = parse_da(&fs::read(command.da)?)?;
    let da = file
        .hwcode_mut(soc.as_hwcode())
        .ok_or(Error::Custom("hwcode not found in the DA".into()))?;
    if !command.skip_patch {
        let asm = Assembler::try_new()?;
        let disasm = Disassembler::try_new()?;

        let da1 = da.da1_mut().ok_or(Error::Custom("DA1 not found".into()))?;
        println!("Patching da1...");
        do_patch(da1.code_mut(), da_patcher::da::DA::hardcoded(&asm, &disasm));
        do_patch(da1.code_mut(), da_patcher::da::DA::security(&asm, &disasm));

        let da2 = da.da2_mut().ok_or(Error::Custom("DA2 not found".into()))?;
        println!("Patching da2...");
        do_patch(da2.code_mut(), da_patcher::da::DA::hardcoded(&asm, &disasm));
        do_patch(da2.code_mut(), da_patcher::da::DA::security(&asm, &disasm));
    }

    let da1 = da.da1().ok_or(Error::Custom("please".into()))?;
    let da2 = da.da2().ok_or(Error::Custom("please".into()))?;

    let da1code = da1.data();
    let da2code = da2.data();

    let addr = soc.da_sram_addr();
    log!("Uploading da1...");
    status!(
        SendDA::new(
            addr,
            da1code.len() as u32,
            da1.signature().len() as u32,
            da1code
        )
        .run(&mut port)
    )?;

    log!("Jumping to da1...");
    status!(JumpDA::new(addr).run(&mut port))?;

    let mut da1info = DA1Setup::new();
    log!("Setting up da1...");
    status!(da1info.run(&mut port))?;
    println!("DA v{}.{}", da1info.major(), da1info.minor());

    if let Some(exploit) = command.exploit {
        let payload = &da1_payload()?;
        do_run_exploit(
            &mut port,
            exploit.map_to_exploit(da1code, da2code, payload, soc)?,
        )?;
    }

    log!("Booting da2...");
    port.write_u32_be(*da2.base())?;
    port.write_u32_be(da2code.len() as u32)?;
    port.write_u32_be(0x1000)?;
    if port.read_u8()? != 0x5a {
        return Err(Error::Custom("DA2 setup is not accepted".into()));
    }

    let chunk_size = 0x1000;
    let chunks = da2code.len() / chunk_size;

    for i in 0..chunks {
        port.write_all(&da2code[i * chunk_size..(i + 1) * chunk_size])?;
        if port.read_u8()? != 0x5a {
            return Err(Error::Custom("DA2 data is not accepted".into()));
        }
    }

    if da2code.len() % chunk_size != 0 {
        port.write_all(&da2code[chunks * chunk_size..])?;
    }

    Ok(())
}

#[inline]
fn do_patch<T: Patch>(data: &mut [u8], patches: Vec<T>) {
    for i in patches {
        match i.patch(data) {
            Ok(()) => println!("{}", i.on_success().green()),
            Err(e) => println!("{}: {e}", i.on_failure().red()),
        }
    }
}

fn do_run_exploit(port: &mut Port, exploit: Exploits) -> Result<()> {
    println!("{} run...", exploit.description());
    exploit.run(port)
}

fn da1_payload() -> Result<Cow<'static, [u8]>> {
    #[cfg(not(feature = "static"))]
    {
        Ok(Cow::Owned(fs::read("target/armv7a-none-eabi/release/da1")?))
    }
    #[cfg(feature = "static")]
    {
        Ok(Cow::Borrowed(include_bytes!(
            "../../../../target/armv7a-none-eabi/release/da1"
        )))
    }
}
