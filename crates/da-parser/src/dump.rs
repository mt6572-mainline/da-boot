use std::{fs, path::PathBuf};

use clap::{Parser, Subcommand};
use clap_num::maybe_hex;
use da_parser::{DA, Result, err::Error, parse_da, parse_lk};

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

fn save(da: DA, output: &PathBuf) -> Result<()> {
    println!("{}", da);
    for (i, region) in da.regions.iter().enumerate() {
        if i == 0 {
            continue;
        }
        fs::write(
            output.join(format!("{:#x}_da{}.bin", da.hw_code, i)),
            &region.code,
        )?;
    }

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
            let vec = parse_da(&data)?;
            if let Some(hw_code) = hw_code {
                let da = vec
                    .into_iter()
                    .find(|da| da.hw_code == hw_code)
                    .ok_or(Error::Custom("Given HW code is not found".into()))?;
                save(da, &cli.output)?;
            } else {
                for da in vec {
                    save(da, &cli.output)?;
                }
            }
        }

        Target::LK => {
            let lk = parse_lk(&data)?;
            println!("{}", lk);
            fs::write(cli.output.join("lk.bin"), lk.code)?;
        }
    }

    Ok(())
}
