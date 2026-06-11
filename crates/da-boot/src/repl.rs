use std::iter::once;

use clap::{Parser, Subcommand};
use clap_num::maybe_hex;
use da_protocol::{Message, Protocol};
use rustyline::{DefaultEditor, error::ReadlineError};

use crate::{Port, Result, boot::rpc::ext::HostExtensions};

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
}

fn do_send(protocol: &mut Protocol<Port>, message: Message) -> Result<()> {
    protocol
        .send_message(&message)
        .inspect(|()| println!("=> {message}"))
        .inspect_err(|e| eprintln!("Failed to send message: {e}"))
        .map_err(Into::into)
}

pub fn run_repl(mut protocol: Protocol<Port>) -> Result<()> {
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

                match REPL::try_parse_from(once("repl").chain(line.split_whitespace())) {
                    Ok(repl) => match repl.command {
                        Command::Ack => {
                            do_send(&mut protocol, Message::Ack)?;
                            print_response(&mut protocol)?;
                        }
                        Command::FlushCache { addr, size } => {
                            do_send(&mut protocol, Message::FlushCache { addr, size })?;
                            print_response(&mut protocol)?;
                        }
                        Command::Jump { addr, r0, r1 } => {
                            do_send(&mut protocol, Message::Jump { addr, r0, r1 })?;
                            print_response(&mut protocol)?;
                        }
                        Command::Reset => {
                            do_send(&mut protocol, Message::Reset)?;
                            print_response(&mut protocol)?;
                        }
                        Command::Read { addr, size } => {
                            println!("Reading {size} bytes from {addr:#010x}...");
                            match protocol.download(addr, size) {
                                Ok(data) => {
                                    println!("<= Downloaded {} bytes", data.len());
                                    hex_dump(&data);
                                }
                                Err(e) => eprintln!("Download failed: {e}"),
                            }
                        }
                        Command::Write { addr, data } => {
                            println!("Writing {} bytes to {addr:#010x}...", data.len());
                            match protocol.upload(addr, &data) {
                                Ok(()) => println!("<= Upload finished"),
                                Err(e) => eprintln!("Upload failed: {e}"),
                            }
                        }
                    },
                    Err(e) => {
                        e.print().ok();
                    }
                }
            }
            Err(ReadlineError::Eof) | Err(ReadlineError::Interrupted) => break,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}

fn print_response(protocol: &mut Protocol<Port>) -> Result<()> {
    match protocol.read_response() {
        Ok(r) => {
            println!("<= {r}");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to read response: {e}");
            Err(e.into())
        }
    }
}

fn hex_dump(data: &[u8]) {
    for chunk in data.chunks(16) {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("{:48} | {}", hex.join(" "), ascii);
    }
}

fn hex_u8(s: &str) -> core::result::Result<u8, String> {
    let s = s
        .strip_prefix("0x")
        .ok_or("byte must be 0x-prefixed hex (e.g. 0x1f)")?;

    u8::from_str_radix(s, 16).map_err(|_| format!("invalid hex byte: 0x{s}"))
}
