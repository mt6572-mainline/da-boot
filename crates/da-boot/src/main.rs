#![feature(try_find)]

use std::{io::Write, path::PathBuf, thread::sleep, time::Duration};

use bincode::Encode;
use clap::{Parser, Subcommand, ValueEnum};
use clap_num::maybe_hex;
use colored::Colorize;
use da_soc::SoC;
use derive_ctor::ctor;
use derive_more::IsVariant;
use serialport::{SerialPortInfo, SerialPortType, available_ports};
use simpleport::{Port, SimpleRead, SimpleWrite};

use crate::{
    boot::{
        bootrom::run_brom,
        preloader::{invalidate_ready, mt6572_preloader_workaround, run_preloader},
    },
    commands::{
        generic::{GetHwCode, GetTargetConfig},
        preloader::{JumpDA, SendDA},
    },
    err::Error,
};

mod boot;
mod commands;
mod err;
mod logging;
mod repl;

type Result<T> = core::result::Result<T, Error>;

const BOOT_ARG_ADDR: u32 = 0x800d0000;

#[derive(Clone, Default, PartialEq, Eq, IsVariant, Subcommand)]
enum BootMode {
    /// Run the binary after BootROM: BootROM -> payload -> your binary
    ///
    /// or the other way: BootROM -> Preloader -> crash -> BootROM -> payload -> your binary
    ///
    /// for U-Boot SPL/other SRAM-only binary testing
    BootROM,
    /// Run the binary after Preloader: BootROM -> Preloader -> payload -> your binary
    ///
    /// for U-Boot testing
    #[default]
    Preloader,
    /// Run the binary after LK: BootROM -> Preloader -> payload -> LK -> your binary
    ///
    /// for U-Boot chainloading
    LK {
        /// LK boot mode
        mode: LkBootMode,
    },
    /// Stay in the payload in the REPL mode
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

    #[command(subcommand)]
    mode: BootMode,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, ValueEnum)]
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
            boot_reason: 4,
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
struct Context {
    pub soc: SoC,
    pub cli: Cli,
}

fn get_ports() -> Result<impl Iterator<Item = (DeviceMode, SerialPortInfo)>> {
    Ok(available_ports()?
        .into_iter()
        .filter_map(|s| match &s.port_type {
            SerialPortType::UsbPort(p) => {
                if p.vid == 0x0e8d {
                    if p.pid == 0x2000 {
                        Some((DeviceMode::Preloader, s))
                    } else if p.pid == 0x0003 {
                        Some((DeviceMode::Brom, s))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        }))
}

fn open_port() -> Result<(DeviceMode, Port)> {
    log!("Waiting for the device");
    let (mode, port) = loop {
        if let Some(port) = get_ports()?.next() {
            println!("");
            break port;
        } else {
            log!(".");
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

fn handshake(port: &mut Port) -> Result<()> {
    loop {
        port.write_u8(0xa0)?;
        port.flush()?;

        if port.read_u8()? == 0x5f {
            break;
        }
    }

    for byte in [0x0a, 0x50, 0x05] {
        port.write_u8(byte)?;
    }

    /* Clean garbage because we spam with handshake  */
    sleep(Duration::from_millis(200));
    port.clear(serialport::ClearBuffer::All)?;

    Ok(())
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

    let state = Context::new(
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
