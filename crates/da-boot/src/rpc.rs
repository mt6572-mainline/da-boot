use da_protocol::{Message, Protocol};
use simpleport::Port;

use crate::{Result, err::Error};

pub trait HostExtensions {
    fn start(&mut self) -> Result<()>;
    fn upload(&mut self, addr: u32, data: &[u8]) -> Result<()>;
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
            self.send_message(Message::write(addr + (i * data.len()) as u32, data))?;
            if self.read_response()?.is_nack() {
                return Err(Error::Custom(
                    format!("Device didn't accept chunk {i}").into(),
                ));
            }
        }

        Ok(())
    }
}
