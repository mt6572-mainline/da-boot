use std::{fs, path::PathBuf};

use clap::{Parser, ValueEnum};
use da_patcher::{Assembler, Disassembler, PatchCollection, Result, da::DA, preloader::Preloader};

#[derive(Clone, ValueEnum)]
enum Type {
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
    ty: Type,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut bytes = fs::read(cli.input)?;
    let asm = Assembler::try_new()?;
    let disasm = Disassembler::try_new()?;

    match cli.ty {
        Type::Preloader => {
            for i in [
                Preloader::security(&asm, &disasm),
                Preloader::hardcoded(&asm, &disasm),
            ]
            .iter()
            .flatten()
            {
                match i.patch(&mut bytes) {
                    Ok(()) => println!("{}", i.on_success()),
                    Err(e) => println!("{}: {}", i.on_failure(), e),
                }
            }
        }
        Type::DA => {
            for i in DA::hardcoded(&asm, &disasm) {
                match i.patch(&mut bytes) {
                    Ok(()) => println!("{}", i.on_success()),
                    Err(e) => println!("{}: {}", i.on_failure(), e),
                }
            }
        }
    }

    fs::write(cli.output, bytes)?;

    Ok(())
}
