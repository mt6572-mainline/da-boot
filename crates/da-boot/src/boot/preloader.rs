use acon::Memory;
use anyhow::{Context, Result};
use da_params::MemoryRange;
use da_patcher::{Extract, preloader::usb_ptr::PreloaderDLULPtr};

use crate::{
    DeviceMode, Port, State, boot::rpc::selector::run_rpc_preloader, get_hwcode, handshake,
    open_port,
};

pub fn run_preloader(state: &mut State, port: Port, device_mode: DeviceMode) -> Result<()> {
    assert!(device_mode.is_preloader());

    let port = mt6572_preloader_workaround(port)?;

    let start = state.soc.dram_start();
    state.params.memory = MemoryRange::new(start, start + (512 * 1024 * 1024));
    let (ptr_dl, ptr_ul) = PreloaderDLULPtr::new(&state.preloader.analyzer)
        .extract()
        .context("Failed to extract Preloader function pointers")?;
    state.params.ptr_dl = ptr_dl;
    state.params.ptr_ul = ptr_ul;

    run_rpc_preloader(state, port).context("Error on RPC run")
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
