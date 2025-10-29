use std::{fs, path::PathBuf};

use clap::Parser;
use da_patcher::{Assembler, Disassembler, PatchCollection, Result, preloader::Preloader};

#[derive(Parser)]
struct Cli {
    /// Preloader file
    #[arg(short, long)]
    input: PathBuf,
    /// Output
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut preloader = fs::read(cli.input)?;
    let asm = Assembler::try_new()?;
    let disasm = Disassembler::try_new()?;

    for i in [
        Preloader::security(&asm, &disasm),
        Preloader::hardcoded(&asm, &disasm),
    ]
    .iter()
    .flatten()
    {
        match i.patch(&mut preloader) {
            Ok(()) => println!("{}", i.on_success()),
            Err(e) => println!("{}: {}", i.on_failure(), e),
        }
    }

    fs::write(cli.output, preloader)?;

    Ok(())
}
