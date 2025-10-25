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
use derive_more::IsVariant;
use serialport::{SerialPort, SerialPortInfo, SerialPortType, available_ports};
use shared::PRELOADER_BASE;

use crate::{
    commands::{GetTargetConfig, JumpDA, Read32, SendDA},
    err::Error,
};

mod commands;
mod err;
mod logging;

type Result<T> = core::result::Result<T, Error>;
type Port = Box<dyn SerialPort>;

const HANDSHAKE: [u8; 3] = [0x0a, 0x50, 0x05];

const DA_SRAM_ADDR: u32 = 0x2007000;
const DA_DRAM_ADDR: u32 = 0x81e00000;
const BOOT_ARG_ADDR: u32 = 0x800d0000;

trait DA {
    fn write_and_check(&mut self, byte: u8, expected: u8) -> Result<bool>;
}

impl DA for Port {
    fn write_and_check(&mut self, byte: u8, expected: u8) -> Result<bool> {
        self.write_all(&[byte])?;
        let mut buf = [0; 1];
        self.read_exact(&mut buf)?;
        Ok(u8::from_be_bytes(buf) == expected)
    }
}

#[derive(Clone, IsVariant, Subcommand)]
enum Command {
    /// Boot bare-metal payload through send_da and jump_da preloader commands
    Boot {
        /// Binaries to upload
        #[arg(short, long, value_delimiter = ' ', num_args = 1..)]
        input: Vec<PathBuf>,

        /// Addresses for binaries
        #[arg(short, long, value_delimiter = ' ', num_args = 1.., value_parser=maybe_hex::<u32>)]
        upload_address: Vec<u32>,

        /// Final jump address, jumps to 0x81e00000 if not set
        #[arg(short, long, value_parser=maybe_hex::<u32>)]
        jump_address: Option<u32>,

        /// Payload boot mode
        #[arg(short, long)]
        mode: Option<Mode>,

        /// LK boot mode
        #[arg(long)]
        lk_mode: Option<LkBootMode>,
    },

    /// Boot preloader patcher and dump preloader with changes (debugging)
    DumpPreloader,
}

#[derive(Clone, Default, ValueEnum, IsVariant)]
#[clap(rename_all = "kebab_case")]
enum Mode {
    #[default]
    Raw,
    Lk,
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

    /// Force skip preloader stage and directly boot the binary in the brom mode
    #[arg(long)]
    only_brom: bool,

    /// Preloader path
    #[arg(short, long)]
    preloader: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
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
        Err(Error::Io(_)) => Ok(()),
        _ => Err(Error::Custom("Retry".into())),
    }
}

fn handshake(port: &mut Port) -> Result<()> {
    let mut buf = [0; 1];
    loop {
        port.write(&[0xa0])?;
        port.flush()?;
        port.read_exact(&mut buf)?;

        if buf[0] == 0x5f {
            break;
        }
    }

    for byte in HANDSHAKE {
        port.write_and_check(byte, !byte)?;
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

fn get_da_addr(mode: DeviceMode) -> u32 {
    match mode {
        DeviceMode::Brom => DA_SRAM_ADDR,
        DeviceMode::Preloader => DA_DRAM_ADDR,
    }
}

fn run(mut cli: Cli, previous_mode: Option<DeviceMode>) -> Result<()> {
    let (device_mode, mut port) = open_port()?;

    if device_mode.is_preloader() {
        /* Read "READY", just to be safe let's expect it may appear up to 4 times */
        let mut buf = [0; 20];
        let _ = port.read(&mut buf);
    }
    handshake(&mut port)?;

    let mut payload = GetTargetConfig::new();
    // mt6572 workaround
    if let Err(_) = payload.run(&mut port) {
        drop(port);
        return run(cli, previous_mode);
    }

    let (sbc, sla, daa) = payload.parse();
    y_n_reverse!("SBC enabled", sbc);
    y_n_reverse!("SLA enabled", sla);
    y_n_reverse!("DAA enabled", daa);

    let protected = sbc || sla || daa;
    if device_mode.is_preloader() && (protected || cli.crash || cli.only_brom) {
        if protected {
            println!("Device is protected, trying brom mode");
        }

        log!("Crashing to brom mode...");
        status!(crash_to_brom(&mut port))?;
        drop(port);
        sleep(Duration::from_millis(100));
        println!();
        return run(cli, Some(device_mode));
    } else if device_mode.is_brom() && protected {
        return Err(Error::Custom(
            "Device is protected, kamakiri is required :(".into(),
        ));
    }

    if let Some(pmode) = previous_mode {
        if pmode.is_brom() && device_mode.is_preloader() && !protected {
            println!("Successfully booted patched preloader through brom");
        }
    }

    let patcher = get_patcher(device_mode);
    let (no_patcher, payload) = match &cli.command {
        Command::Boot {
            upload_address,
            input,
            ..
        } => {
            let da_addr = upload_address[0];
            // Without patcher we can upload only one binary
            let no_patcher = upload_address.len() == 1
                // If we're in preloader mode, address must match DRAM addr
                && ((da_addr == DA_DRAM_ADDR && device_mode.is_preloader())
                    // If we're in brom mode, address must match SRAM addr
                    || (da_addr == DA_SRAM_ADDR && device_mode.is_brom()))
                // We must not force patcher
                && !cli.force
                // BROM patcher disables restriction for the payload size,
                // so we can boot preloader and possibly U-Boot SPL
                && device_mode.is_preloader();
            (
                no_patcher,
                fs::read(if no_patcher { &input[0] } else { patcher })?,
            )
        }
        Command::DumpPreloader => (false, fs::read(patcher)?),
    };

    if device_mode.is_preloader() && no_patcher {
        println!(
            "Preloader won't be patched, some commands may be not available due to security checks"
        );
    }

    let da_addr = get_da_addr(device_mode);
    log!("Uploading payload to {da_addr:#x}...");
    status!(SendDA::new(da_addr, payload.len() as u32, 0, &payload).run(&mut port))?;
    log!("Jumping to {da_addr:#x}...");
    status!(JumpDA::new(da_addr).run(&mut port))?;

    if device_mode.is_brom() {
        log!("Trying to sync with brom payload...");
        port.write_all(&u32::to_be_bytes(0x1337))?;
        let mut buf = [0; 2];
        port.read_exact(&mut buf)?;
        if u16::from_be_bytes(buf) != 0x1337 {
            return Err(Error::Custom("Failed syncing with brom".into()));
        }
        println!("ok");

        if cli.only_brom {
            return Ok(());
        }

        let needs_dram_init = match &cli.command {
            Command::Boot {
                upload_address,
                jump_address,
                ..
            } => {
                upload_address.iter().any(|u| *u > DA_SRAM_ADDR)
                    || jump_address.is_some_and(|j| j > DA_SRAM_ADDR)
            }
            Command::DumpPreloader => true,
        };
        if needs_dram_init {
            println!("DRAM init is required for the specified payload");

            let mut payload = match cli.preloader {
                Some(ref p) => fs::read(p)?,
                None => return Err(Error::Custom("Preloader is required from DRAM init".into())),
            };
            payload.truncate(131 * 1024);
            let pad = payload.len() % 4;
            if pad != 0 {
                for _ in 0..4 - pad {
                    payload.push(0);
                }
            }

            log!("Booting preloader at {PRELOADER_BASE:#x}...");
            port.write_all(&(PRELOADER_BASE as u32).to_be_bytes())?;
            port.write_all(&(payload.len() as u32).to_be_bytes())?;
            port.write_all(&payload)?;

            port.read_exact(&mut buf)?;
            if u16::from_be_bytes(buf) != 0 {
                return Err(Error::Custom(
                    "Failed uploading payload in the brom mode".into(),
                ));
            }
            println!("ok");
            println!("Jumping to {PRELOADER_BASE:#x}...");
            cli.crash = false;
            drop(port);
            sleep(Duration::from_millis(100));
            println!();
            return run(cli, previous_mode);
        }
    }

    if !no_patcher {
        log!("Trying to sync with patched preloader...");
        status!(handshake(&mut port))?;
    }

    match cli.command {
        Command::Boot {
            input,
            upload_address,
            jump_address,
            mode,
            lk_mode,
        } => {
            if !no_patcher {
                let mode = mode.unwrap_or_default();

                for (i, a) in input.into_iter().zip(upload_address) {
                    let mut payload = fs::read(i)?;
                    if mode.is_lk() {
                        payload.drain(0..0x200);
                    }
                    log!("Uploading payload to {a:#x}...");
                    status!(SendDA::new(a, payload.len() as u32, 0, &payload).run(&mut port))?;
                }

                if mode.is_lk() {
                    log!("Preparing boot argument for LK...");
                    let payload = bincode::encode_to_vec(
                        BootArgument::lk(lk_mode.unwrap_or_default()),
                        bincode::config::standard()
                            .with_little_endian()
                            .with_fixed_int_encoding(),
                    )?;
                    status!(
                        SendDA::new(BOOT_ARG_ADDR, payload.len() as u32, 0, &payload)
                            .run(&mut port)
                    )?;
                }

                let jump = jump_address.unwrap_or(get_da_addr(device_mode));
                log!("Jumping to {jump:#x}...");
                status!(JumpDA::new(jump).run(&mut port))?;
            }
        }
        Command::DumpPreloader => {
            log!("Dumping preloader from ram...");
            let mut payload = Read32::new(0x2007500, (1 * 1024 * 1024) / 4);
            status!(payload.run(&mut port))?;
            let preloader = payload
                .buf
                .into_iter()
                .map(|u32| u32.to_le_bytes())
                .flatten()
                .collect::<Vec<_>>();
            fs::write("preloader.bin", preloader)?;
            return Ok(());
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Boot {
            input,
            upload_address,
            ..
        } => {
            assert!(!input.is_empty());
            assert_eq!(input.len(), upload_address.len());
        }
        _ => (),
    }

    println!("For BROM mode short KCOL0 to the GND or add the crash option and connect the device");
    println!("For preloader mode simply connect the device");
    run(cli, None)
}
