use std::{
    borrow::Borrow,
    iter::{self, once},
};

use clap::{Parser, Subcommand};
use clap_num::maybe_hex;
use da_protocol::{Message, Protocol};
use rustyline::{DefaultEditor, error::ReadlineError};

use crate::Result;

#[derive(Parser)]
struct REPL {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
#[repr(u8)]
enum Command {
    /// Heartbeat.
    Ack = 0x42,
    /// Read data at `addr` with `size` length.
    Read {
        #[arg(value_parser=maybe_hex::<u32>)]
        addr: u32,
        #[arg(value_parser=maybe_hex::<u32>)]
        size: u32,
    },
    /// Write `data` to `addr`.
    Write {
        #[arg(value_parser=maybe_hex::<u32>)]
        addr: u32,
        #[arg(num_args=1.., value_parser=hex_u8)]
        data: Vec<u8>,
    },
    /// Flush I and D-cache at `addr` with `size` aligned to 64.
    FlushCache {
        #[arg(value_parser=maybe_hex::<u32>)]
        addr: u32,
        #[arg(value_parser=maybe_hex::<u32>)]
        size: u32,
    },
    /// Jump to `addr`. The `addr` **must** contain **ARM** mode instructions.
    Jump {
        #[arg(value_parser=maybe_hex::<u32>)]
        addr: u32,
        #[arg(value_parser=maybe_hex::<u32>)]
        r0: Option<u32>,
        #[arg(value_parser=maybe_hex::<u32>)]
        r1: Option<u32>,
    },
    /// Reset the device using watchdog.
    Reset,

    // Preloader commands
    /// Return to `usbdl_handler`.
    Return,
}

impl Command {
    fn as_message<'a>(&'a self) -> Message<'a> {
        match self {
            Self::Ack => Message::Ack,
            Self::Read { addr, size } => Message::Read {
                addr: *addr,
                size: *size,
            },
            Self::Write { addr, data } => Message::Write {
                addr: *addr,
                data: data,
            },
            Self::FlushCache { addr, size } => Message::FlushCache {
                addr: *addr,
                size: *size,
            },
            Self::Jump { addr, r0, r1 } => Message::Jump {
                addr: *addr,
                r0: *r0,
                r1: *r1,
            },
            Self::Reset => Message::Reset,
            Self::Return => Message::Return,
        }
    }
}

pub fn run_repl(mut protocol: Protocol<simpleport::Port, 2048>) -> Result<()> {
    println!("Enter --help for help, Ctrl-C to exit");

    let mut rl = DefaultEditor::new()?;

    loop {
        let line = rl.readline("> ").map(|l| l.trim().to_owned());
        match line {
            Ok(line) => {
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(&line)?;

                match REPL::try_parse_from(once("repl").chain(line.split(" "))) {
                    Ok(repl) => {
                        let message = repl.command.as_message();
                        match protocol.send_message(&message) {
                            Ok(_) => println!("=> {message}"),
                            Err(e) => {
                                eprintln!("Failed to send message: {e}");
                                Err(e)?
                            }
                        }

                        match protocol.read_response() {
                            Ok(r) => println!("<= {r}"),
                            Err(e) => Err(e)?,
                        }
                    }
                    Err(e) => {
                        e.print().ok();
                    }
                }
            }
            Err(ReadlineError::Eof) | Err(ReadlineError::Interrupted) => break,
            Err(e) => Err(e)?,
        }
    }

    Ok(())
}

fn hex_u8(s: &str) -> core::result::Result<u8, String> {
    let s = s
        .strip_prefix("0x")
        .ok_or("byte must be 0x-prefixed hex (e.g. 0x1f)")?;

    u8::from_str_radix(s, 16).map_err(|_| format!("invalid hex byte: 0x{s}"))
}
