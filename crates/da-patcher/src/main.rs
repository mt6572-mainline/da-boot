use std::{fs, path::PathBuf};

use clap::{Parser, ValueEnum};
use clap_num::maybe_hex;
use da_analyzer::Analyzer;
use da_patcher::{
    Assembler, Patch, Result,
    da::{hash::Hash, uart_port::UartPort},
    oneshot,
    preloader::hw_check_battery::HwCheckBattery,
};

#[derive(Clone, ValueEnum)]
enum Mode {
    Preloader,
    DA,
}

#[derive(Parser)]
struct Cli {
    /// Preloader file
    #[arg(short, long)]
    input: PathBuf,
    /// Output
    #[arg(short, long)]
    output: PathBuf,

    /// Binary type
    #[arg(short, long)]
    mode: Mode,

    /// Base address
    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    addr: u32,
}

pub fn print_oneshot<'a, T: Patch<'a>>(
    asm: &'a Assembler,
    analyzer: &'a Analyzer,
    bytes: &mut [u8],
) {
    match oneshot::<T>(asm, analyzer, bytes) {
        Ok(()) => println!("{} is patched", T::name()),
        Err(e) => println!("{} is NOT patched: {e}", T::name()),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut bytes = fs::read(cli.input)?;
    let asm = Assembler::try_new()?;
    let analyzer = Analyzer::try_new(
        bytes.clone(),
        cli.addr as usize,
        da_analyzer::cpu_mode::CpuMode::Arm,
    )?;

    match cli.mode {
        Mode::Preloader => {
            print_oneshot::<HwCheckBattery>(&asm, &analyzer, &mut bytes);
        }
        Mode::DA => {
            print_oneshot::<Hash>(&asm, &analyzer, &mut bytes);
            print_oneshot::<UartPort>(&asm, &analyzer, &mut bytes);
        }
    }

    fs::write(cli.output, bytes)?;

    Ok(())
}
