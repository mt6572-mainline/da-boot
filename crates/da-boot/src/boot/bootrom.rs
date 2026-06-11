use std::{thread::sleep, time::Duration};

use anyhow::{Context, Result};
use da_params::MemoryRange;
use da_protocol::Message;

use crate::{
    BootMode, DeviceMode, Port, State,
    boot::{
        preloader::{invalidate_ready, run_preloader},
        rpc::{
            ext::HostExtensions,
            selector::{brom_payload, inject_params, start_rpc},
        },
    },
    handshake, open_port, run_payload,
};

pub fn run_brom(state: &mut State, mut port: Port, device_mode: DeviceMode) -> Result<()> {
    assert!(device_mode.is_brom());

    // BUG: 0x2000000~0x2001000 is unusable.
    state.params.memory = MemoryRange::new(0x2001000, 0x2020000);
    state.params.ptr_dl = 0x40B9C4 | 1;
    state.params.ptr_ul = 0x40BA4A | 1;

    for image in &state.upload {
        let addr = image.upload_address();
        if state
            .params
            .blacklist_reloc(addr..addr + image.len() as u32 + 1)
            .is_err()
        {
            anyhow::bail!("Failed blacklisting range: {addr:#x}");
        }

        println!("Reserved memory: {addr:#x}");
    }

    let addr = state.preloader.file.upload_address();
    if state
        .params
        .blacklist_reloc(addr..addr + state.preloader.file.len() as u32 + 1)
        .is_err()
    {
        anyhow::bail!("Failed blacklisting range: {addr:#x}");
    }

    println!("Reserved memory: {addr:#x}");

    let mut payload = brom_payload()?;
    let mut payload = payload.to_mut();
    inject_params(&state, &mut payload)?;
    run_payload(0x2001000, &payload, &mut port)?;
    let mut protocol = start_rpc(port)?;

    println!("Got loader sync !");

    match state.mode {
        BootMode::BootROM => {
            if state.upload.is_empty() {
                anyhow::bail!("No binary to boot");
            }

            for image in &state.upload {
                let addr = image.upload_address();
                println!("Uploading image to {addr:#x}");
                protocol
                    .upload(addr, &image)
                    .context("Failed uploading image")?;
                protocol
                    .send_message(Message::BlacklistRange(addr..addr + image.len() as u32 + 1))?;
                if !protocol.read_response().is_ok_and(|r| r.is_ack()) {
                    anyhow::bail!("Failed blacklisting {addr:#x}");
                }
            }

            Ok(())
        }
        _ => {
            let preloader = &state.preloader;
            println!(
                "Booting preloader at {:#x}",
                preloader.file.upload_address()
            );
            protocol
                .upload(preloader.file.upload_address(), &preloader.file)
                .context("Error on sending Preloader")?;

            println!("Jump to preloader");
            protocol.send_message(Message::jump(preloader.file.upload_address(), None, None))?;
            if protocol.read_response().is_ok_and(|r| r.is_nack()) {
                anyhow::bail!("Error on jumping to Preloader");
            }

            drop(protocol);
            sleep(Duration::from_millis(100));
            println!();

            let (device_mode, mut port) = open_port()?;
            invalidate_ready(&mut port)?;
            handshake(&mut port)?;
            run_preloader(state, port, device_mode).context("Error on Preloader run")
        }
    }
}
