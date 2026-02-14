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

    let analyzer = Analyzer::new(&data);
    let idx = analyzer.find_string_ref(&cli.s).unwrap();

    println!("guessed function code:");
    for i in analyzer.find_function_bounds(idx).unwrap() {
        let (inst, off) = &analyzer.code[i];
        println!("\t{:#x}: {}", off, inst);
    }
}
