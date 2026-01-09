use std::{
    fs,
    io::{Write, stdout},
    path::{Path, PathBuf},
    thread::sleep,
    time::Duration,
};

use bincode::Encode;
use clap::{Parser, Subcommand, ValueEnum};
use clap_num::maybe_hex;
use colored::Colorize;
use da_patcher::{Assembler, Disassembler, Patch as _, PatchCollection, preloader::Preloader};
use da_protocol::{Message, Protocol};
use da_soc::SoC;
use derive_ctor::ctor;
use derive_more::IsVariant;
use serialport::{SerialPortInfo, SerialPortType, available_ports};
use simpleport::{Port, SimpleRead, SimpleWrite};

use crate::{
    commands::{
        custom_brom::{RunPayload, Sync},
        custom_preloader::{DumpPreloader, Patch, Return},
        generic::{GetHwCode, GetTargetConfig},
        preloader::{JumpDA, Read32, SendDA},
    },
    err::Error,
    exploit::{Exploit as _, Exploits},
    repl::run_repl,
    rpc::HostExtensions,
};

mod commands;
mod err;
mod exploit;
mod logging;
mod repl;
mod rpc;

type Result<T> = core::result::Result<T, Error>;

const HANDSHAKE: [u8; 3] = [0x0a, 0x50, 0x05];

const BOOT_ARG_ADDR: u32 = 0x800d0000;

#[derive(Clone, ValueEnum, IsVariant)]
#[clap(rename_all = "kebab_case")]
enum Exploit {
    /// DA2 write32 command abuse
    Croissant,
    /// DA1 function pointer overwrite
    Croissant2,
    /// DA1 hash overwrite
    Pumpkin,
}

#[derive(Clone, Default, ValueEnum, IsVariant)]
#[clap(rename_all = "kebab_case")]
enum Mode {
    #[default]
    Raw,
    Lk,
    REPL,
}

#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Force brom mode
    #[arg(short, long)]
    crash: bool,

    /// Force booting preloader patcher
    #[arg(short, long)]
    force: bool,

    /// Preloader path
    #[arg(short, long)]
    preloader: Option<PathBuf>,

    /// Binaries to upload
    #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
    input: Vec<PathBuf>,

    /// Addresses for binaries
    #[arg(short, long, value_delimiter = ' ', num_args = 1.., value_parser=maybe_hex::<u32>)]
    upload_address: Vec<u32>,

    /// Final jump address, jumps to DA1 DRAM address if not set
    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    jump_address: Option<u32>,

    /// Boot mode
    #[arg(short, long)]
    mode: Option<Mode>,

    /// LK boot mode
    #[arg(long)]
    lk_mode: Option<LkBootMode>,
}

#[derive(Debug, Clone, Default, Encode, ValueEnum)]
#[clap(rename_all = "kebab_case")]
#[repr(u32)]
enum LkBootMode {
    #[default]
    Normal,
    Meta,
    Recovery,
    SwReboot,
    Factory,
    Advmeta,
    AteFactory,
    Alarm,
    Fastboot = 99,
    Download,
}

#[derive(Default, Encode)]
#[repr(C)]
struct BootArgument {
    magic: u32,
    mode: u32,
    e_flag: u32,
    log_port: u32,
    log_baudrate: u32,
    log_enable: u8,
    reserved: [u8; 3],
    dram_rank_num: u32,
    dram_rank_size: [u32; 4],
    boot_reason: u32,
    meta_com_type: u32,
    meta_com_id: u32,
    boot_time: u32,
    /* da_info_t */
    addr: u32,
    arg1: u32,
    arg2: u32,
    /* SEC_LIMIT */
    magic_num: u32,
    forbid_mode: u32,
}

impl BootArgument {
    pub fn lk(mode: LkBootMode) -> Self {
        Self {
            magic: 0x504c504c,
            mode: mode as u32,
            e_flag: 0,
            log_port: 0x11005000,
            log_baudrate: 921600,
            log_enable: 1,
            dram_rank_num: 1,
            dram_rank_size: [0x20000000, 0, 0, 0],
            boot_reason: 1,
            boot_time: 1337,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone, IsVariant)]
enum DeviceMode {
    Brom,
    Preloader,
}

#[derive(ctor)]
struct State {
    pub soc: SoC,
    pub cli: Cli,
}

fn get_ports() -> Result<Vec<(DeviceMode, SerialPortInfo)>> {
    Ok(available_ports()?
        .into_iter()
        .filter_map(|s| match &s.port_type {
            SerialPortType::UsbPort(p) => {
                let is_target = p.pid == 0x2000 || p.pid == 0x0003;
                if p.vid == 0x0e8d && is_target {
                    Some((
                        if p.pid == 0x0003 {
                            DeviceMode::Brom
                        } else {
                            DeviceMode::Preloader
                        },
                        s,
                    ))
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect())
}

fn open_port() -> Result<(DeviceMode, Port)> {
    log!("Waiting for the device");
    let (mode, port) = loop {
        let ports = get_ports()?;

        if ports.len() > 1 {
            return Err(Error::MoreThanOneDevice);
        } else if ports.is_empty() {
            log!(".");
        } else {
            println!("");
            break ports[0].clone();
        }

        sleep(Duration::from_millis(500));
    };

    println!("Found device at {}", &port.port_name);
    Ok((
        mode,
        serialport::new(port.port_name, 921600)
            .timeout(Duration::from_millis(2000))
            .open()?,
    ))
}

fn crash_to_brom(port: &mut Port) -> Result<()> {
    match Read32::new(0x0, 1).run(port) {
        Err(Error::Simpleport(simpleport::err::Error::Io(_))) => Ok(()),
        _ => Err(Error::Custom("Retry".into())),
    }
}

fn handshake(port: &mut Port) -> Result<()> {
    loop {
        port.write_u8(0xa0)?;
        port.flush()?;

        if port.read_u8()? == 0x5f {
            break;
        }
    }

    for byte in HANDSHAKE {
        port.write_u8(byte)?;
    }

    /* Clean garbage because we spam with handshake  */
    sleep(Duration::from_millis(200));
    port.clear(serialport::ClearBuffer::All)?;

    Ok(())
}

fn get_patcher<'a>(mode: DeviceMode) -> &'a Path {
    match mode {
        DeviceMode::Brom => Path::new("target/armv7a-none-eabi/release/brom"),
        DeviceMode::Preloader => Path::new("target/armv7a-none-eabi/release/preloader"),
    }
}

fn get_da_addr(state: &State, mode: DeviceMode) -> u32 {
    match mode {
        DeviceMode::Brom => state.soc.da_sram_addr(),
        DeviceMode::Preloader => state.soc.da_dram_addr(),
    }
}

fn get_hwcode(port: &mut Port) -> Result<u16> {
    GetHwCode::new().run_hwcode(port)
}

fn print_target(port: &mut Port) -> Result<()> {
    let mut payload = GetTargetConfig::new();
    payload.run(port)?;

    let (sbc, sla, daa) = payload.parse();
    y_n_reverse!("SBC enabled", sbc);
    y_n_reverse!("SLA enabled", sla);
    y_n_reverse!("DAA enabled", daa);

    Ok(())
}

fn run_payload(addr: u32, payload: &[u8], port: &mut Port) -> Result<()> {
    log!("Uploading payload to {addr:#x}...");
    status!(SendDA::new(addr, payload.len() as u32, 0, &payload).run(port))?;
    log!("Jumping to {addr:#x}...");
    status!(JumpDA::new(addr).run(port))
}

fn run_brom(mut state: State, mut port: Port, device_mode: DeviceMode) -> Result<()> {
    assert!(device_mode.is_brom());

    run_payload(0x2001000, &fs::read(get_patcher(device_mode))?, &mut port)?;

    let mut protocol = Protocol::new(port, [0; 2048]);

    log!("Trying to sync with brom payload...");
    status!(protocol.start())?;

    let mut payload = fs::read(state.cli.preloader.clone().ok_or(Error::Custom("Preloader is required in the BROM mode, please specify preloader without header via -p option".into()))?)?;
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
    status!(protocol.send_message(Message::jump(
        preloader_base,
        Some(BOOT_ARG_ADDR),
        Some(250)
    )))?;
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
    return run_preloader(state, port, device_mode);
}

fn run_preloader(state: State, port: Port, device_mode: DeviceMode) -> Result<()> {
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

    let da_addr = get_da_addr(&state, device_mode);
    let payload = fs::read(get_patcher(device_mode))?;

    run_payload(da_addr, &payload, &mut port)?;

    let mut protocol = Protocol::new(port, [0; 2048]);
    protocol.start()?;

    let mode = state.cli.mode.unwrap_or_default();

    for (idx, (i, a)) in state
        .cli
        .input
        .into_iter()
        .zip(state.cli.upload_address)
        .enumerate()
    {
        let mut payload = fs::read(i)?;
        if mode.is_lk() && idx == 0 {
            payload.drain(0..0x200);
        }
        log!("Uploading payload to {a:#x}...");
        status!(protocol.upload(a, &payload))?;
    }

    match mode {
        Mode::Raw => (),
        Mode::Lk => {
            log!("Preparing boot argument for LK...");
            let payload = bincode::encode_to_vec(
                BootArgument::lk(state.cli.lk_mode.unwrap_or_default()),
                bincode::config::standard()
                    .with_little_endian()
                    .with_fixed_int_encoding(),
            )?;
            status!(protocol.upload(BOOT_ARG_ADDR, &payload))?;
        }
        Mode::REPL => return run_repl(protocol),
    }

    let jump = state.cli.jump_address.unwrap_or(da_addr);
    log!("Jumping to {jump:#x}...");
    status!(protocol.send_message(Message::jump(jump, Some(BOOT_ARG_ADDR), Some(250))))?;
    if protocol.read_response().is_ok_and(|r| r.is_nack()) {
        Err(Error::Custom("Jump failed".into()))
    } else {
        Ok(())
    }
}

fn invalidate_ready(port: &mut Port) -> Result<()> {
    /* Read "READY", just to be safe let's expect it may appear up to 4 times */
    let mut buf = [0; 20];
    let _ = port.read(&mut buf)?;
    Ok(())
}

fn mt6572_preloader_workaround(mut port: Port) -> Result<Port> {
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

fn run(cli: Cli) -> Result<()> {
    let (device_mode, mut port) = open_port()?;

    if device_mode.is_preloader() {
        invalidate_ready(&mut port)?;
    }

    handshake(&mut port)?;

    let mut port = mt6572_preloader_workaround(port)?;
    let hwcode = get_hwcode(&mut port)?;
    println!("HW code: {hwcode:#x}");

    print_target(&mut port)?;

    let state = State::new(
        SoC::try_from_hwcode(hwcode).ok_or(Error::UnsupportedSoC(hwcode))?,
        cli,
    );
    match device_mode {
        DeviceMode::Brom => run_brom(state, port, device_mode),
        DeviceMode::Preloader => run_preloader(state, port, device_mode),
    }
}

fn main() -> core::result::Result<(), String> {
    let cli = Cli::parse();

    assert!(!cli.input.is_empty());
    assert_eq!(cli.input.len(), cli.upload_address.len());

    println!("For BROM mode short KCOL0 to the GND or add the crash option and connect the device");
    println!("For preloader mode simply connect the device");
    run(cli).map_err(|e| e.to_string())
}
