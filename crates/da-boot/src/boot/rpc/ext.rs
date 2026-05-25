use da_protocol::{Message, Protocol, Response};
use simpleport::Port;

use crate::{Result, err::Error};

pub trait HostExtensions {
    fn start(&mut self) -> Result<()>;
    fn upload(&mut self, addr: u32, data: &[u8]) -> Result<()>;
    fn download(&mut self, addr: u32, len: u32) -> Result<Vec<u8>>;
}

impl<const N: usize> HostExtensions for Protocol<Port, N> {
    fn start(&mut self) -> Result<()> {
        if self.read_message()?.is_ack() {
            self.send_message(Message::ack()).map_err(|e| e.into())
        } else {
            Err(Error::Custom("Device didn't send ACK".into()))
        }
    }

    fn upload(&mut self, addr: u32, data: &[u8]) -> Result<()> {
        for (i, data) in data.chunks(Self::RW_BUFFER_SIZE).enumerate() {
            let addr = addr + (i * Self::RW_BUFFER_SIZE) as u32;
            self.send_message(Message::write(addr, data))?;
            if self.read_response()?.is_nack() {
                return Err(Error::Custom(
                    format!("Device didn't accept chunk {i}").into(),
                ));
            }
            self.send_message(Message::flush_cache(addr, data.len() as u32))?;
            if self.read_response()?.is_nack() {
                return Err(Error::Custom(
                    format!("Device didn't flush cache at chunk {i}").into(),
                ));
            }
        }

        Ok(())
    }

    fn download(&mut self, addr: u32, len: u32) -> Result<Vec<u8>> {
        let mut vec = Vec::with_capacity(len as usize);
        let mut do_download = |addr: u32, len: u32| {
            self.send_message(Message::read(addr, len))?;
            if let Response::Read { data } = self.read_response()? {
                vec.extend_from_slice(data);
                Ok(())
            } else {
                Err(Error::Custom("Device didn't respond with read".into()))
            }
        };

        let rw = Self::RW_BUFFER_SIZE as u32;
        for i in 0..len / rw {
            do_download(addr + (i * rw), rw)?;
        }

        let remainder = len % rw;
        if remainder != 0 {
            let offset = (len / rw) * rw;
            do_download(addr + offset, remainder)?;
        }

        Ok(vec)
    }
}
