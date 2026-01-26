use std::{borrow::Cow, fs};

use da_parser::parse_lk;
use da_protocol::{HookId, Message, Protocol};
use da_soc::SoC;
use simpleport::Port;

use crate::{
    BOOT_ARG_ADDR, BootArgument, CommandBoot, Mode, Result, err::Error, log, repl::run_repl,
    rpc::HostExtensions, run_payload, status,
};

pub fn run_rpc_preloader(soc: SoC, mut port: Port, command: CommandBoot) -> Result<()> {
    let da_addr = soc.da_dram_addr();
    run_payload(da_addr, &rpc_payload()?, &mut port)?;

    let mut protocol = start_rpc(port)?;

    let mode = command.mode.unwrap_or_default();
    let payloads = command
        .input
        .into_iter()
        .map(|i| fs::read(i).map_err(|e| e.into()))
        .collect::<Result<Vec<_>>>()?;

    for (idx, (payload, a)) in payloads
        .iter()
        .zip(command.upload_address.iter())
        .enumerate()
    {
        if mode.is_lk() && idx == 0 {
            let lk = parse_lk(&payload);
            let code = match lk {
                Ok(ref lk) => {
                    println!("\n{lk}");
                    lk.code()
                }
                _ => {
                    eprintln!("LK header detection failed, assuming raw binary");
                    &payload
                }
            };

            log!("Uploading LK to {a:#x}...");
            status!(protocol.upload(*a, code))?;
        } else {
            log!("Uploading payload to {a:#x}...");
            status!(protocol.upload(*a, &payload))?;
        }
    }

    match mode {
        Mode::Raw => (),
        Mode::Lk => {
            log!("Preparing boot argument for LK...");
            let payload = bincode::encode_to_vec(
                BootArgument::lk(command.lk_mode.unwrap_or_default()),
                bincode::config::standard()
                    .with_little_endian()
                    .with_fixed_int_encoding(),
            )?;
            status!(protocol.upload(BOOT_ARG_ADDR, &payload))?;

            if command.upload_address.len() > 1 {
                log!("Setting up LK hooks...");
                protocol.send_message(Message::hook(HookId::MtPartGenericRead))?;
                if !status!(protocol.read_response())?.is_ack() {
                    return Err(Error::Custom("Error on enabling LK hooks".into()));
                }
            }
        }
        Mode::REPL => return run_repl(protocol),
    }

    let jump = command.jump_address.unwrap_or(da_addr);
    log!("Jumping to {jump:#x}...");
    status!(protocol.send_message(Message::jump(jump, Some(BOOT_ARG_ADDR), Some(250))))?;
    if protocol.read_response().is_ok_and(|r| r.is_nack()) {
        Err(Error::Custom("Jump failed".into()))
    } else {
        Ok(())
    }
}

pub fn start_rpc(port: Port) -> Result<Protocol<Port, 2048>> {
    let mut protocol = Protocol::new(port, [0; 2048]);
    status!(protocol.start())?;
    Ok(protocol)
}

pub fn rpc_payload() -> Result<Cow<'static, [u8]>> {
    #[cfg(not(feature = "static"))]
    {
        Ok(Cow::Owned(fs::read("target/armv7a-none-eabi/nostd/rpc")?))
    }
    #[cfg(feature = "static")]
    {
        Ok(Cow::Borrowed(include_bytes!(
            "../../../../target/armv7a-none-eabi/nostd/rpc"
        )))
    }
}
