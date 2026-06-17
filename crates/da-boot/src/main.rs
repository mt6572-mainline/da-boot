use std::{
    io::{Write, stdout},
    ops::Deref,
    thread::sleep,
    time::Duration,
};

use acon::{MMIO, SoC};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use clap_num::maybe_hex;
use da_params::PayloadParams;
use da_patcher::{Extract, preloader::lk_base::LKBase};
use derive_ctor::ctor;
use derive_more::IsVariant;
use hacc::{Image, Preloader, TryRead};
use kaiko::Analyzer;
use serialport::{SerialPort, SerialPortInfo, SerialPortType, available_ports};
use simpleport::{SimpleRead, SimpleWrite};
use which::which;

type Port = Box<dyn SerialPort>;

use crate::{
    boot::{
        bootrom::run_brom,
        lk_arg::LkBootMode,
        preloader::{invalidate_ready, mt6572_preloader_workaround, run_preloader},
    },
    commands::{
        generic::GetHwCode,
        preloader::{JumpDA, Read32, SendDA},
    },
    err::Error,
    file_ext::{FileContent, FileContentSpec, UploadFile, UploadFileSpec},
};

mod boot;
mod commands;
mod err;
mod file_ext;
mod repl;

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
    LK,
    /// Stay in the payload in the REPL mode
    REPL,
}

#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Force brom mode
    #[arg(short, long)]
    crash: bool,

    /// Preloader path
    #[arg(short, long)]
    preloader: FileContentSpec,

    /// Manually specify Preloader address (used only if header autodetection fails)
    #[arg(long)]
    preloader_addr: Option<u32>,

    /// LK path
    #[arg(short, long)]
    lk: Option<FileContentSpec>,

    /// Manually specify LK address (used only if header autodetection fails)
    #[arg(long)]
    lk_addr: Option<u32>,

    /// LK boot mode (used only if mode is preloader or lk)
    #[arg(short, long)]
    lk_mode: Option<LkBootMode>,

    /// DRAM size per rank
    #[arg(long, value_parser=maybe_hex::<u32>)]
    dram_size_per_rank: Option<u32>,

    /// DRAM ranks
    #[arg(long)]
    dram_ranks: Option<u32>,

    /// zImage path
    #[arg(short, long)]
    kernel: Option<FileContentSpec>,

    /// initrd path
    #[arg(short, long, requires = "kernel")]
    ramdisk: Option<FileContentSpec>,

    /// Binaries to upload
    #[arg(short, long, num_args = 1..)]
    input: Vec<UploadFileSpec>,

    /// Final jump address if booting binary from the `input`
    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    jump_address: Option<u32>,

    #[command(subcommand)]
    mode: BootMode,
}

#[derive(Debug, Copy, Clone, IsVariant)]
enum DeviceMode {
    Brom,
    Preloader,
}

#[derive(ctor)]
struct FileAndAnalyzer {
    file: UploadFile,
    analyzer: Analyzer,
}

#[derive(ctor)]
struct LKState {
    file_and_analyzer: FileAndAnalyzer,
}

impl Deref for LKState {
    type Target = FileAndAnalyzer;

    fn deref(&self) -> &Self::Target {
        &self.file_and_analyzer
    }
}

struct State {
    pub soc: SoC,

    mode: BootMode,
    lk_mode: LkBootMode,

    dram_size_per_rank: u32,
    dram_ranks: u32,

    upload: Vec<UploadFile>,
    preloader: FileAndAnalyzer,
    lk: Option<LKState>,
    // kernel and ramdisk have fixed addr in the LK
    //
    // XXX: this may be not true on newer SoCs
    kernel: Option<FileContent>,
    ramdisk: Option<FileContent>,
    // jump address provided by the CLI.
    //
    // unused if not booting image from `upload`
    jump_addr: u32,

    params: PayloadParams,
}

fn get_ports() -> Result<impl Iterator<Item = (DeviceMode, SerialPortInfo)>> {
    Ok(available_ports()
        .context("no ports available")?
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
    print!("Waiting for the device...");
    let (mode, port) = loop {
        if let Some(port) = get_ports()?.next() {
            println!();
            break port;
        } else {
            print!(".");
            stdout().flush()?;
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
        port.write_u8(0xa0).context("error on sending sync byte")?;
        port.flush()?;

        if port.read_u8().context("error on syncing with the device")? == 0x5f {
            break;
        }
    }

    for byte in [0x0a, 0x50, 0x05] {
        port.write_u8(byte).context("error on handshake")?;
    }

    /* Clean garbage because we spam with handshake  */
    sleep(Duration::from_millis(200));
    port.clear(serialport::ClearBuffer::All)?;

    Ok(())
}

fn get_hwcode(port: &mut Port) -> Result<u16, Error> {
    GetHwCode::new().run_hwcode(port)
}

fn run_payload(addr: u32, payload: &[u8], port: &mut Port) -> Result<()> {
    println!("Sending to {addr:#x}");
    SendDA::new(addr, payload.len() as u32, 0, &payload)
        .run(port)
        .context("Error on sending payload")?;
    println!("Jump to {addr:#x}");
    JumpDA::new(addr)
        .run(port)
        .context("Error on jumping to the payload")
}

fn crash_to_brom(port: &mut Port) -> Result<()> {
    match Read32::new(0x0, 1).run(port) {
        Err(Error::Io(_)) => Ok(()),
        _ => anyhow::bail!("Device didn't crash, is brom usbdl disabled?"),
    }
}

fn run(mut state: State, crash: bool) -> Result<()> {
    let (device_mode, mut port) = open_port()?;

    if device_mode.is_preloader() {
        invalidate_ready(&mut port)?;
    }

    handshake(&mut port)?;

    let mut port = mt6572_preloader_workaround(port)?;
    let hwcode = get_hwcode(&mut port).context("Error on getting hwcode")?;
    println!("HW code: {hwcode:#x}");

    let soc = SoC::try_from_hwcode(hwcode).context("Sorry, your SoC is not supported yet")?;
    state.soc = soc;
    state.params.soc = soc;

    match device_mode {
        DeviceMode::Brom => run_brom(&mut state, port, device_mode).context("Error on BootROM run"),
        DeviceMode::Preloader => {
            if crash {
                crash_to_brom(&mut port).context("Error on crashing to BootROM")?;
                drop(port);
                sleep(Duration::from_millis(100));
                println!();

                let (device_mode, mut port) = open_port()?;
                handshake(&mut port)?;
                run_brom(&mut state, port, device_mode)
            } else {
                run_preloader(&mut state, port, device_mode).context("Error on Preloader run")
            }
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut params = PayloadParams::default();

    if !cli.preloader.is_file() {
        anyhow::bail!("Preloader file doesn't exist");
    }

    let preloader_file = FileContent::try_from(cli.preloader).context("Can't read preloader")?;
    let (pl, pl_base) = if let Ok(pl) = Preloader::try_read(&preloader_file) {
        let pl_jump = pl.gfh().file_info().load_addr() + pl.gfh().file_info().jump_offset();

        // wow. mtk bullshit is everywhere.
        let max = pl.content().len();
        let mut content = preloader_file.into_vec();
        if content.starts_with(b"EMMC_BOOT") {
            content.drain(0..0xb00);
        } else if content.starts_with(b"MMM") {
            content.drain(0..0x300);
        } else {
            anyhow::bail!("Junk preloader");
        }

        content.resize(max, 0);

        (FileContent::from(content), pl_jump)
    } else {
        if let Some(addr) = cli.preloader_addr {
            println!("Failed to parse preloader, assuming raw file");
            (preloader_file, addr)
        } else {
            anyhow::bail!(
                "Preloader appears to be without the header, jump address must be provided using CLI argument instead"
            );
        }
    };
    println!("Loaded preloader ({} bytes, base: {pl_base:#x})", pl.len());
    let pl_data = UploadFile::from_content(pl, pl_base);
    let pl_analyzer = Analyzer::try_new(
        pl_data.as_vec().clone().into_boxed_slice(),
        pl_base,
        0,
        kaiko::cpu_mode::CpuMode::Arm,
    )
    .context("Failed to analyze preloader")?;
    let pl = FileAndAnalyzer::new(pl_data, pl_analyzer);

    let lk = if let Some(lk) = cli.lk {
        let content = FileContent::try_from(lk).context("Can't read LK")?;
        let image = Image::new(&content);

        let content = if let Some(part) = image.partitions().next() {
            println!("Loaded {} partition", part.header.name());
            FileContent::from(part.content.to_vec())
        } else {
            println!("Failed to parse LK, assuming raw file");
            content
        };

        let lk_base = LKBase::new(&pl.analyzer)
            .extract()
            .context("Failed to extract LK data")?;

        println!("Loaded LK ({} bytes, base: {lk_base:#x})", content.len());

        // better safe than sorry
        let bss = lk_base + content.len() as u32;
        if params.blacklist_reloc(bss..bss + (512 * 1024)).is_err() {
            anyhow::bail!("Failed to blacklist LK BSS range");
        }

        let analyzer = Analyzer::try_new(
            content.as_vec().clone().into_boxed_slice(),
            lk_base,
            0,
            kaiko::cpu_mode::CpuMode::Arm,
        )
        .context("Failed to analyze LK")?;
        let file = UploadFile::from_content(content, lk_base);

        let f_and_a = FileAndAnalyzer::new(file, analyzer);
        Some(LKState::new(f_and_a))
    } else {
        None
    };

    let kernel = if let Some(kernel) = cli.kernel {
        Some(FileContent::try_from(kernel).context("Can't read kernel")?)
    } else {
        None
    };

    let ramdisk = if let Some(ramdisk) = cli.ramdisk {
        Some(FileContent::try_from(ramdisk).context("Can't read ramdisk")?)
    } else {
        None
    };

    let input = cli
        .input
        .into_iter()
        .map(|f| UploadFile::try_from(f))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .context("Can't read file")?;

    let has_at_least_one_file = !input.is_empty();
    let has_jump = cli.jump_address.is_some();
    let has_kernel = kernel.is_some();
    match cli.mode {
        // BootROM needs Preloader, the preloader path is already checked
        BootMode::BootROM => {
            if has_kernel {
                anyhow::bail!("Booting kernel is not possible in the BootROM mode");
            } else if !has_at_least_one_file {
                println!("BootROM mode will boot Preloader");
            }

            if !has_jump {
                anyhow::bail!(
                    "BootROM needs target jump address if the target image is not Preloader"
                );
            }
        }
        // Preloader needs... preloader, and the path is already checked
        BootMode::Preloader => {
            if !has_at_least_one_file && lk.is_none() {
                // XXX: boot stock LK
                anyhow::bail!("Preloader needs at least one file to boot");
            } else if !has_jump && lk.is_none() {
                anyhow::bail!("Preloader needs target jump address");
            } else if has_kernel {
                anyhow::bail!("Booting kernel is not possible in the Preloader mode");
            }
        }
        // LK needs Preloader and the LK, as well as at least one file to boot
        BootMode::LK { .. } => {
            if lk.is_none() {
                anyhow::bail!("LK mode requires LK file");
            } else if !has_at_least_one_file && kernel.is_none() {
                anyhow::bail!("LK mode requires kernel or prepared image");
            } else if has_jump {
                // XXX: remove once we can check if LK hardcodes kernel addr or no
                //
                // the mt6572 hardcodes it, so we can't have jump addr
                anyhow::bail!("LK mode can't have jump address");
            }

            if has_at_least_one_file && kernel.is_none() {
                println!("Using prepared file, mkbootimg won't be invoked");
            } else if which("mkbootimg").is_err() {
                anyhow::bail!("mkbootimg is not installed for the LK mode");
            }

            if cli.dram_size_per_rank.is_none() || cli.dram_ranks.is_none() {
                anyhow::bail!("Unknown DRAM size. Please provide DRAM rank size and rank count");
            }
        }
        // REPL doesn't really need anything except preloader
        BootMode::REPL => {
            if has_jump {
                anyhow::bail!("REPL mode can't have jump address");
            } else if has_kernel {
                anyhow::bail!("Booting kernel is not possible in the REPL mode");
            }
        }
    }

    let state = State {
        soc: SoC::MT6572,
        mode: cli.mode,
        lk_mode: cli.lk_mode.unwrap_or_default(),
        dram_size_per_rank: cli.dram_size_per_rank.unwrap_or_default(),
        dram_ranks: cli.dram_ranks.unwrap_or_default(),
        upload: input,
        preloader: pl,
        lk,
        kernel,
        ramdisk,
        jump_addr: cli.jump_address.unwrap_or_default(),
        params,
    };

    println!("For BROM mode short KCOL0 to GND or add the crash option and connect the device");
    println!("For preloader mode simply connect the device");
    println!();
    run(state, cli.crash)
}
