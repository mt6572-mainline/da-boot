use std::{fs, path::PathBuf};

use clap::Parser;
use clap_num::maybe_hex;
use da_parser::{DA, Result, err::Error, parse};

#[derive(Parser)]
struct Cli {
    /// DA file
    #[arg(short, long)]
    input: PathBuf,
    /// Output directory
    #[arg(short, long)]
    output: PathBuf,
    /// Filter SoC by HW code
    #[arg(long, value_parser=maybe_hex::<u16>)]
    hw_code: Option<u16>,
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

    let vec = parse(&data)?;
    if let Some(hw_code) = cli.hw_code {
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

    Ok(())
}
