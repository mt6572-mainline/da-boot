use std::{fs, path::PathBuf};

use clap::Parser;
use da_analyzer::{Analyzer, cpu_mode::CpuMode};

#[derive(Parser)]
struct Cli {
    /// Input file
    #[arg(short, long)]
    input: PathBuf,

    /// String to search for
    #[arg(short, long)]
    s: String,

    /// Binary base address
    #[arg(short, long)]
    base: usize,
}

fn main() {
    let cli = Cli::parse();
    let data = fs::read(cli.input).unwrap();

    println!("analyzing code flow");
    let analyzer = Analyzer::try_new(data, cli.base, CpuMode::Arm).unwrap();
    dbg!(analyzer.find_string_ref(&cli.s));
}
