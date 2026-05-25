use std::{borrow::Cow, fs};

use da_protocol::{HookId, Message, Protocol};
use da_soc::SoC;
use simpleport::Port;

use crate::{
    BOOT_ARG_ADDR, BootArgument, BootMode, Context, Result, boot::rpc::ext::HostExtensions,
    err::Error, log, repl::run_repl, run_payload, status,
};

pub fn run_rpc_preloader(context: Context, mut port: Port) -> Result<()> {
    let da_addr = context.soc.da_dram_addr();
    run_payload(da_addr, &rpc_payload()?, &mut port)?;

    let mut protocol = start_rpc(port)?;

    let payloads = context
        .cli
        .input
        .into_iter()
        .map(|i| fs::read(i).map_err(|e| e.into()))
        .collect::<Result<Vec<_>>>()?;

    for (idx, (payload, a)) in payloads
        .iter()
        .zip(context.cli.upload_address.iter())
        .enumerate()
    {
        if context.cli.mode.is_lk() && idx == 0 {
            log!("Uploading LK to {a:#x}...");
            status!(protocol.upload(*a, &payload))?;
        } else {
            log!("Uploading payload to {a:#x}...");
            status!(protocol.upload(*a, &payload))?;
        }
    }

    match context.cli.mode {
        BootMode::BootROM | BootMode::Preloader => (),
        BootMode::LK { mode } => {
            log!("Preparing boot argument for LK...");
            let payload = bincode::encode_to_vec(
                BootArgument::lk(mode),
                bincode::config::standard()
                    .with_little_endian()
                    .with_fixed_int_encoding(),
            )?;
            status!(protocol.upload(BOOT_ARG_ADDR, &payload))?;

            if context.cli.upload_address.len() > 1 {
                log!("Setting up LK hooks...");
                protocol.send_message(Message::hook(HookId::MtPartGenericRead))?;
                if !status!(protocol.read_response())?.is_ack() {
                    return Err(Error::Custom(
                        "Error on enabling mt_part_generic_read".into(),
                    ));
                }
                protocol.send_message(Message::Hook(HookId::MbootAndroidCheckImgInfo))?;
                if !status!(protocol.read_response())?.is_ack() {
                    return Err(Error::Custom(
                        "Error on enabling mboot_android_check_img_info".into(),
                    ));
                }
            }
        }
        BootMode::REPL => return run_repl(protocol),
    }

    let jump = context.cli.jump_address.unwrap_or(da_addr);
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
