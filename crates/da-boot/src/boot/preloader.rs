use std::{thread::sleep, time::Duration};

use simpleport::Port;

use crate::{
    Command, DeviceMode, Result, State,
    boot::{bootrom::run_brom, da::run_da1, rpc::run_rpc_preloader},
    commands::preloader::Read32,
    err::Error,
    get_hwcode, handshake, log, open_port, status,
};

pub fn run_preloader(state: State, port: Port, device_mode: DeviceMode) -> Result<()> {
    assert!(device_mode.is_preloader());

    let mut port = mt6572_preloader_workaround(port)?;

    if state.cli.crash {
        log!("Crashing to brom mode...");
        status!(crash_to_brom(&mut port))?;
        drop(port);
        sleep(Duration::from_millis(100));
        println!();

        let (device_mode, mut port) = open_port()?;
        handshake(&mut port)?;
        return run_brom(state, port, device_mode);
    }

    match state.cli.command {
        Command::Boot(command) => run_rpc_preloader(state.soc, port, command),
        Command::DA(command) => run_da1(state.soc, port, command),
    }
}

pub fn invalidate_ready(port: &mut Port) -> Result<()> {
    /* Read "READY", just to be safe let's expect it may appear up to 4 times */
    let mut buf = [0; 20];
    let _ = port.read(&mut buf)?;
    Ok(())
}

pub fn mt6572_preloader_workaround(mut port: Port) -> Result<Port> {
    if let Err(_) = get_hwcode(&mut port) {
        drop(port);
        let (_, mut port) = open_port()?;
        invalidate_ready(&mut port)?;
        handshake(&mut port)?;
        Ok(port)
    } else {
        Ok(port)
    }
}

fn crash_to_brom(port: &mut Port) -> Result<()> {
    match Read32::new(0x0, 1).run(port) {
        Err(Error::Simpleport(simpleport::err::Error::Io(_))) => Ok(()),
        _ => Err(Error::Custom("Retry".into())),
    }
}
