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
use da_parser::parse_da;
use da_patcher::{Assembler, Disassembler, Patch as _, PatchCollection, preloader::Preloader};
use da_protocol::{Port, SimpleRead, SimpleWrite};
use da_soc::SoC;
use derive_ctor::ctor;
use derive_more::IsVariant;
use serialport::{SerialPortInfo, SerialPortType, available_ports};
use sha1::{Digest, Sha1};
use shared::PRELOADER_BASE;

use crate::{
    commands::{
        custom_brom::{RunPayload, Sync},
        custom_preloader::{DumpPreloader, Patch, Return},
        da::{DA1Setup, DA2Ack},
        generic::{GetHwCode, GetTargetConfig},
        preloader::{JumpDA, Read32, SendDA},
    },
    err::Error,
};

mod commands;
mod err;
mod logging;

type Result<T> = core::result::Result<T, Error>;

const HANDSHAKE: [u8; 3] = [0x0a, 0x50, 0x05];

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

        /// Final jump address, jumps to DA1 DRAM address if not set
        #[arg(short, long, value_parser=maybe_hex::<u32>)]
        jump_address: Option<u32>,

        /// Payload boot mode
        #[arg(short, long)]
        mode: Option<Mode>,

        /// LK boot mode
        #[arg(long)]
        lk_mode: Option<LkBootMode>,
    },

    /// Boot DA
    BootDA {
        /// DA file
        #[arg(short, long)]
        input: PathBuf,
        /// Do not patch the DA even if the device is not protected
        #[arg(long)]
        quirky_preloader: bool,
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

#[derive(ctor)]
struct State {
    pub soc: SoC,
    pub cli: Cli,
    pub is_preloader_patched: bool,
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
        Err(Error::DAProtocol(da_protocol::err::Error::Io(_))) => Ok(()),
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

    run_payload(
        get_da_addr(&state, device_mode),
        &fs::read(get_patcher(device_mode))?,
        &mut port,
    )?;

    log!("Trying to sync with brom payload...");
    status!(Sync::new(0x1337).run(&mut port))?;

    let mut payload = fs::read(state.cli.preloader.clone().ok_or(Error::Custom("Preloader is required in the BROM mode, please specify preloader without header via -p option".into()))?)?;
    payload.truncate(131 * 1024);
    let pad = payload.len() % 4;
    if pad != 0 {
        for _ in 0..4 - pad {
            payload.push(0);
        }
    }

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

    log!("Booting preloader at {PRELOADER_BASE:#x}...");
    status!(RunPayload::new(PRELOADER_BASE as u32, payload.len() as u32, &payload).run(&mut port))?;
    println!("Jumping to {PRELOADER_BASE:#x}...");

    state.cli.crash = false;
    state.is_preloader_patched = true;

    drop(port);
    sleep(Duration::from_millis(100));
    println!();

    let (device_mode, mut port) = open_port()?;
    invalidate_ready(&mut port)?;
    handshake(&mut port)?;
    return run_preloader(state, port, device_mode);
}

fn run_preloader(mut state: State, port: Port, device_mode: DeviceMode) -> Result<()> {
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
    let (run_patcher, payload) = match &state.cli.command {
        Command::Boot {
            input,
            upload_address,
            jump_address,
            mode,
            ..
        } => {
            // For LK mode we always need patched preloader
            let patcher_required = mode.as_ref().is_some_and(|m| m.is_lk())
                // or force option is enabled
                || state.cli.force
                // or binaries to upload is more than 1
                || upload_address.len() > 1
                // or upload address is not equal to the hardcoded one
                || upload_address[0] != da_addr
                // or jump address is not equal to the hardcoded one
                || jump_address.unwrap_or(da_addr) != da_addr;
            (
                patcher_required,
                fs::read(if patcher_required {
                    get_patcher(device_mode)
                } else {
                    &input[0]
                })?,
            )
        }
        // For dumping preloader we need read32 patched
        Command::DumpPreloader => (true, fs::read(get_patcher(device_mode))?),
        Command::BootDA {
            input,
            quirky_preloader,
        } => return run_da(&state, port, input, !quirky_preloader),
    };

    // This will run either preloader patcher or actual payload
    if !run_patcher && !state.is_preloader_patched {
        return Ok(());
    }

    if state.is_preloader_patched {
        println!("Successfully booted patched preloader through BROM mode");
    }

    match &state.cli.command {
        Command::Boot { mode, input, .. } if input.len() > 1 => {
            println!("We still need preloader patcher to boot hook LK");
            state.is_preloader_patched = false;
        }
        _ => (),
    }

    if !state.is_preloader_patched {
        run_payload(da_addr, &payload, &mut port)?;
    }

    if !state.is_preloader_patched {
        let mut payload = match &state.cli.preloader {
            Some(p) => fs::read(p)?,
            None => {
                log!("No preloader specified, dumping from RAM...");
                status!(DumpPreloader::new().run_preloader(&mut port))?
            }
        };

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
            let offset = match i.offset(&payload) {
                Ok(offset) => offset,
                Err(e) => {
                    println!("{}: {e}", i.on_failure().red());
                    continue;
                }
            };

            let replacement = i.replacement(&mut payload)?;

            if replacement.len() % 2 != 0 {
                return Err(Error::Custom(
                    "Replacement is not aligned to 2, please fix da-patcher".into(),
                ));
            }

            match Patch::new(
                (PRELOADER_BASE + offset) as u32,
                replacement.len() as u32,
                &replacement,
            )
            .run(&mut port)
            {
                Ok(_) => println!("{}", i.on_success().green()),
                Err(e) => println!("{}: {e}", i.on_failure().red()),
            }
        }

        log!("Jumping back to usbdl_handler...");
        status!(Return::new().run(&mut port))?;

        log!("Trying to sync with patched preloader...");
        status!(handshake(&mut port))?;

        state.is_preloader_patched = true;
    }

    match state.cli.command {
        Command::Boot {
            input,
            upload_address,
            jump_address,
            mode,
            lk_mode,
        } => {
            let mode = mode.unwrap_or_default();

            for (idx, (i, a)) in input.into_iter().zip(upload_address).enumerate() {
                let mut payload = fs::read(i)?;
                if mode.is_lk() && idx == 0 {
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
                    SendDA::new(BOOT_ARG_ADDR, payload.len() as u32, 0, &payload).run(&mut port)
                )?;
            }

            let jump = jump_address.unwrap_or(da_addr);
            log!("Jumping to {jump:#x}...");
            status!(JumpDA::new(jump).run(&mut port))?;
        }

        Command::DumpPreloader => {
            log!("Dumping preloader from ram...");
            let preloader = status!(
                Read32::new(PRELOADER_BASE as u32, (1 * 1024 * 1024) / 4).run_buf(&mut port)
            )?
            .into_iter()
            .map(|u32| u32.to_le_bytes())
            .flatten()
            .collect::<Vec<_>>();
            fs::write("preloader.bin", preloader)?;
            return Ok(());
        }
        _ => unreachable!(),
    }

    Ok(())
}

fn run_da(state: &State, mut port: Port, input: &PathBuf, patch_da: bool) -> Result<()> {
    let mut da = parse_da(&fs::read(input)?)?
        .into_iter()
        .find(|da| da.hw_code == 0x6572)
        .ok_or(Error::Custom("Invalid DA file".into()))?;

    let (da1, da2) = da.regions.split_at_mut(2);

    println!("\nDA1: {}", da1[1]);
    println!("\nDA2: {}\n", da2[0]);

    let da1 = &mut da1[1];
    let da2 = &mut da2[0];
    let mut da1code = &mut da1.code;
    let mut da2code = &mut da2.code;

    if patch_da {
        let asm = Assembler::try_new()?;
        let disasm = Disassembler::try_new()?;
        println!("Patching da1...");
        for i in [
            // FIXME: add switch to disable hash check or update hash
            // not sure which one would be better for now
            //da_patcher::da::DA::security(&asm, &disasm),
            da_patcher::da::DA::hardcoded(&asm, &disasm),
        ]
        .iter()
        .flatten()
        {
            match i.patch(&mut da1code) {
                Ok(()) => println!("{}", i.on_success().green()),
                Err(e) => println!("{}: {e}", i.on_failure().red()),
            }
        }

        let mut hasher = Sha1::new();
        hasher.update(&da2code[..da2code.len() - 256]);
        let hash = hasher.finalize();
        println!("da2 hash: {hash:#x}");

        let index = (0..da1code.len())
            .find(|&i| da1code[i..].starts_with(&hash))
            .ok_or(Error::Custom("da2 hash not found :(".into()))?;

        println!("Patching da2...");
        for i in [
            da_patcher::da::DA::security(&asm, &disasm),
            da_patcher::da::DA::hardcoded(&asm, &disasm),
        ]
        .iter()
        .flatten()
        {
            match i.patch(&mut da2code) {
                Ok(()) => println!("{}", i.on_success().green()),
                Err(e) => println!("{}: {e}", i.on_failure().red()),
            }
        }

        let mut hasher = Sha1::new();
        hasher.update(&da2code[..da2code.len() - 256]);
        let replacement = hasher.finalize();
        println!("patched da2 hash: {replacement:#x}");

        da1code[index..index + 20].clone_from_slice(&replacement);
    }

    let da_addr = state.soc.da_dram_addr();
    if state.soc.is_da1_addr_hardcoded_in_preloader() {
        log!(
            "Uploading da1 to {da_addr:#x} (ignored, upload address is overwritten by preloader)..."
        );
    } else {
        log!("Uploading da1 to {da_addr:#x}...");
    }

    status!(
        SendDA::new(da_addr, da1code.len() as u32, da1.signature_size, &da1code).run(&mut port)
    )?;
    log!("Jumping to {da_addr:#x}...");
    status!(JumpDA::new(da_addr).run(&mut port))?;

    log!("Setting up da1...");
    let mut da1info = DA1Setup::new();
    status!(da1info.run(&mut port))?;
    println!("DA v{}.{}", da1info.major(), da1info.minor());

    log!("Booting da2...");
    port.write_u32(da2.base)?;
    port.write_u32(da2code.len() as u32)?;
    port.write_u32(0x1000)?;
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

    status!(DA2Ack::new(0x5a, 0x5a).run(&mut port))?;

    println!("DA2 is up and running");

    Ok(())
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
        false,
    );
    match device_mode {
        DeviceMode::Brom => run_brom(state, port, device_mode),
        DeviceMode::Preloader => run_preloader(state, port, device_mode),
    }
}

fn main() -> core::result::Result<(), String> {
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
    run(cli).map_err(|e| e.to_string())
}
