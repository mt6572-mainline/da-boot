use anyhow::{Context, Result};
use da_protocol::{Message, Protocol};
use kdam::{BarExt, tqdm};

use crate::Port;

const CHUNK_SIZE: usize = 256 * 1024;

pub trait HostExtensions {
    fn start(&mut self) -> Result<()>;
    fn upload(&mut self, addr: u32, data: &[u8]) -> Result<()>;
    fn download(&mut self, addr: u32, len: u32) -> Result<Vec<u8>>;
}

impl HostExtensions for Protocol<Port> {
    fn start(&mut self) -> Result<()> {
        if self.read_message()?.is_ack() {
            self.send_message(Message::ack()).map_err(|e| e.into())
        } else {
            anyhow::bail!("device didn't reply with ack");
        }
    }

    fn upload(&mut self, addr: u32, data: &[u8]) -> Result<()> {
        let mut pb = tqdm!(total = data.len(), desc = format!("{addr:#x}"), unit = "B");

        for (i, data) in data.chunks(CHUNK_SIZE).enumerate() {
            let addr = addr + (i * CHUNK_SIZE) as u32;
            self.send_message(Message::write(addr, data.len() as u32))?;
            self.io.write_all(data)?;
            self.read_response()?;
            pb.update(data.len())?;
        }

        Ok(())
    }

    fn download(&mut self, addr: u32, len: u32) -> Result<Vec<u8>> {
        let mut vec = vec![0u8; len as usize];
        let mut do_download = |addr: u32, chunk_len: u32, offset: usize| {
            self.send_message(Message::read(addr, chunk_len))?;

            let start = offset;
            let end = start + chunk_len as usize;
            self.io.read_exact(&mut vec[start..end])?;

            if self.read_response()?.is_ack() {
                Ok(())
            } else {
                anyhow::bail!("device didn't reply with ack");
            }
        };

        let chunk_size = CHUNK_SIZE as u32;
        for i in 0..len / chunk_size {
            let offset = (i * chunk_size) as usize;
            do_download(addr + (i * chunk_size), chunk_size, offset)
                .with_context(|| format!("error on sending chunk {i}"))?;
        }

        let remainder = len % chunk_size;
        if remainder != 0 {
            let offset = ((len / chunk_size) * chunk_size) as usize;
            do_download(addr + offset as u32, remainder, offset)
                .context("error on sending remainder")?;
        }

        Ok(vec)
    }
}
