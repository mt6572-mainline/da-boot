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
}

fn main() {
    let cli = Cli::parse();
    let data = fs::read(cli.input).unwrap();

    let analyzer = Analyzer::new_thumb(&data);
    let idx = analyzer.find_string_ref(&cli.s).unwrap();

    println!("guessed function code:");
    for i in analyzer.find_function_bounds(idx).unwrap() {
        println!("\t{:#x}: {}", i.offset(), i.instruction());
    }

    println!("basic blocks:");
    let blocks = analyzer
        .find_basic_blocks(analyzer.find_function_bounds(idx).unwrap())
        .unwrap();
    for (i, block) in blocks.iter().enumerate() {
        println!("block {i}:");
        for i in block.iter() {
            println!("\t{:#x}: {}", i.offset(), i.instruction());
        }
    }
}
