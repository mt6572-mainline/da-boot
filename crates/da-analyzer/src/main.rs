use std::{fs, path::PathBuf};

use clap::Parser;
use da_analyzer::Analyzer;

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

    let analyzer = Analyzer::new_thumb(data, cli.base);
    let idx = analyzer.find_string_ref(&cli.s).unwrap();

    println!("basic blocks:");
    for (i, block) in analyzer.analyze_function(idx).unwrap().iter().enumerate() {
        println!("block {i}:");
        for i in block.code().iter() {
            println!("\t{:#x}: {}", i.offset(), i.instruction());
        }
    }
}
