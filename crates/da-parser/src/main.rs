use std::{
    fs,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use clap_num::maybe_hex;
use da_parser::{Result, da::hl::Entry, err::Error, parse_da, parse_lk};

#[derive(Subcommand)]
enum Target {
    LK,
    DA {
        /// Filter SoC by HW code
        #[arg(long, value_parser=maybe_hex::<u16>)]
        hw_code: Option<u16>,
    },
}

#[derive(Parser)]
struct Cli {
    /// Input file
    #[arg(short, long)]
    input: PathBuf,
    /// Output directory
    #[arg(short, long)]
    output: PathBuf,

    #[command(subcommand)]
    target: Target,
}

fn save_da(output: &Path, hwcode: u16, entry: &Entry) -> Result<()> {
    let da1 = entry.da1().ok_or(Error::Custom("DA1 not found".into()))?;
    fs::write(output.join(format!("{hwcode:#06X}-da1.bin")), da1.code())?;
    fs::write(
        output.join(format!("{hwcode:#06X}-da1.sig.bin")),
        da1.signature(),
    )?;

    let da2 = entry.da2().ok_or(Error::Custom("DA2 not found".into()))?;
    fs::write(output.join(format!("{hwcode:#06X}-da2.bin")), da2.code())?;
    fs::write(
        output.join(format!("{hwcode:#06X}-da2.sig.bin")),
        da2.signature(),
    )?;

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data = fs::read(cli.input)?;
    if !cli.output.exists() {
        return Err(Error::Custom("Output directory doesn't exist".into()));
    }

    match cli.target {
        Target::DA { hw_code } => {
            let da = parse_da(&data)?;
            if let Some(hw_code) = hw_code {
                let entry = da
                    .hwcode(hw_code)
                    .ok_or(Error::Custom("HW code not found".into()))?;
                println!("{entry}");
                save_da(&cli.output, hw_code, entry)?;
            } else {
                println!("{da}");
                da.entries()
                    .iter()
                    .try_for_each(|entry| save_da(&cli.output, *entry.hw_code(), entry))?
            }
        }

        Target::LK => {
            let lk = parse_lk(&data)?;
            println!("{lk}");
            fs::write(cli.output.join("lk.bin"), lk.code())?;
        }
    }

    Ok(())
}
