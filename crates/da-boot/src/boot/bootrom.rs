use std::{fs, thread::sleep, time::Duration};

use colored::Colorize;
use da_parser::preloader_header_size;
use da_patcher::{Assembler, Disassembler, Patch, PatchCollection, preloader::Preloader};
use da_protocol::Message;
use simpleport::Port;

use crate::{
    DeviceMode, Result, State,
    boot::{
        preloader::{invalidate_ready, run_preloader},
        rpc::{rpc_payload, start_rpc},
    },
    err::Error,
    handshake, log, open_port,
    rpc::HostExtensions,
    run_payload, status,
};

pub fn run_brom(mut state: State, mut port: Port, device_mode: DeviceMode) -> Result<()> {
    assert!(device_mode.is_brom());

    run_payload(0x2001000, &rpc_payload()?, &mut port)?;
    let mut protocol = start_rpc(port)?;

    let mut payload = if let Some(ref preloader) = state.cli.preloader {
        let mut payload = fs::read(preloader)?;
        let header = preloader_header_size(&payload).unwrap_or_else(|_| {
            eprintln!("Preloader header detection failed, assuming raw binary");
            0
        });

        payload.drain(0..header);
        payload
    } else {
        return Err(Error::Custom(
            "Preloader is required in the BROM mode, please specify preloader without header via -p option".into(),
        ));
    };

    payload.truncate(100 * 1024);

    let asm = Assembler::try_new()?;
    let disasm = Disassembler::try_new()?;

    println!("Patching preloader...");
    for i in [
        Preloader::security(&asm, &disasm),
        Preloader::hardcoded(&asm, &disasm),
    ]
    .iter()
    .flatten()
    {
        match i.patch(&mut payload) {
            Ok(()) => println!("{}", i.on_success().green()),
            Err(e) => println!("{}: {e}", i.on_failure().red()),
        }
    }

    let preloader_base = state.soc.preloader_addr();

    log!("Booting preloader at {preloader_base:#x}...");
    status!(protocol.upload(preloader_base, &payload))?;

    log!("Jumping to {preloader_base:#x}...");
    status!(protocol.send_message(Message::jump(preloader_base, None, None)))?;
    if protocol.read_response().is_ok_and(|r| r.is_nack()) {
        return Err(Error::Custom("Jump failed".into()));
    }

    state.cli.crash = false;

    drop(protocol);
    sleep(Duration::from_millis(100));
    println!();

    let (device_mode, mut port) = open_port()?;
    invalidate_ready(&mut port)?;
    handshake(&mut port)?;
    run_preloader(state, port, device_mode)
}
