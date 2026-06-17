use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use clap_num::maybe_hex;
use da_cli_ext::{maybe_image, maybe_preloader};
use da_patcher::{
    Extract,
    lk::{
        get_part::GetPart, mt_part_generic_read::MtPartGenericRead,
        mt_part_get_partition::MtPartGetPartition,
    },
    preloader::{bldr_jump::BldrJump, lk_base::LKBase, usb_ptr::PreloaderDLULPtr},
};
use kaiko::Analyzer;

#[derive(Clone, ValueEnum)]
enum Mode {
    Preloader,
    LK,
}

#[derive(Parser)]
struct Cli {
    /// Input file
    #[arg(short, long)]
    input: PathBuf,

    /// Base address
    #[arg(short, long, value_parser=maybe_hex::<u32>)]
    base: u32,

    /// Target mode
    #[arg(short, long)]
    mode: Mode,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data = fs::read(cli.input)?;

    match cli.mode {
        Mode::Preloader => {
            let content = maybe_preloader(&data)
                .inspect(|(addr, _)| println!("Loaded preloader at {addr:#x}"))
                .map(|(_, d)| d)
                .unwrap_or(&data);

            let analyzer =
                Analyzer::try_new(content.into(), cli.base, 0, kaiko::cpu_mode::CpuMode::Arm)?;

            let (dl, ul) = PreloaderDLULPtr::new(&analyzer)
                .extract()
                .context("error on extracting pointers")?;
            println!("Preloader DL: {dl:#x}, UL: {ul:#x}");

            let lk_base = LKBase::new(&analyzer)
                .extract()
                .context("error on extracting lk memory")?;
            println!("LK base: {lk_base:#x}");

            let (bldr_jump, da_addr) = BldrJump::new(&analyzer)
                .extract()
                .context("error on extracting bldr_jump ptr")?;
            println!("bldr_jump: {bldr_jump:#x}, DA DRAM addr: {da_addr:#x}");
        }
        Mode::LK => {
            let content = maybe_image(&data)
                .inspect(|(s, _)| println!("Loaded image {s}"))
                .map(|(_, d)| d)
                .unwrap_or(&data);

            let analyzer =
                Analyzer::try_new(content.into(), cli.base, 0, kaiko::cpu_mode::CpuMode::Arm)?;

            match MtPartGetPartition::new(&analyzer).extract() {
                Ok(v) => println!("mt_part_get_partition: {v:#x}"),
                Err(e) => eprintln!("failed to find mt_part_get_partition: {e:?}"),
            }

            match GetPart::new(&analyzer).extract() {
                Ok(v) => println!("get_part: {v:#x}"),
                Err(e) => eprintln!("failed to find get_part: {e:?}"),
            }

            match MtPartGenericRead::new(&analyzer).extract() {
                Ok(v) => println!("mt_part_generic_read: {v:#x}"),
                Err(e) => eprintln!("failed to find mt_part_generic_read: {e:?}"),
            }
        }
    }

    Ok(())
}
