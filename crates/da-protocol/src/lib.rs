use std::io::{Read, Write};

use serialport::SerialPort;

use crate::err::Error;

pub mod err;

type Result<T> = core::result::Result<T, Error>;
pub type Port = Box<dyn SerialPort>;

pub trait FromBeBytes<const N: usize> {
    fn from_be(bytes: [u8; N]) -> Self;
}

pub trait ToBeBytes<const N: usize> {
    fn to_be(&self) -> [u8; N];
}

pub trait SimpleRead: Write {
    fn simple_read<T: FromBeBytes<N>, const N: usize>(&mut self) -> Result<T>;

    fn read_u8(&mut self) -> Result<u8> {
        self.simple_read()
    }

    fn read_u16(&mut self) -> Result<u16> {
        self.simple_read()
    }

    fn read_u32(&mut self) -> Result<u32> {
        self.simple_read()
    }
}

pub trait SimpleWrite {
    fn simple_write<T: ToBeBytes<N>, const N: usize>(&mut self, value: T) -> Result<()>;

    fn write_u8(&mut self, value: u8) -> Result<()> {
        self.simple_write(value)
    }

    fn write_u16(&mut self, value: u16) -> Result<()> {
        self.simple_write(value)
    }

    fn write_u32(&mut self, value: u32) -> Result<()> {
        self.simple_write(value)
    }
}

impl FromBeBytes<1> for u8 {
    fn from_be(bytes: [u8; 1]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

impl ToBeBytes<1> for u8 {
    fn to_be(&self) -> [u8; 1] {
        self.to_be_bytes()
    }
}

impl FromBeBytes<2> for u16 {
    fn from_be(bytes: [u8; 2]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

impl ToBeBytes<2> for u16 {
    fn to_be(&self) -> [u8; 2] {
        self.to_be_bytes()
    }
}

impl FromBeBytes<4> for u32 {
    fn from_be(bytes: [u8; 4]) -> Self {
        Self::from_be_bytes(bytes)
    }
}

impl ToBeBytes<4> for u32 {
    fn to_be(&self) -> [u8; 4] {
        self.to_be_bytes()
    }
}

impl SimpleRead for Port {
    fn simple_read<T: FromBeBytes<N>, const N: usize>(&mut self) -> Result<T> {
        let mut bytes = [0; N];
        self.read_exact(&mut bytes)?;
        Ok(T::from_be(bytes))
    }
}

impl SimpleWrite for Port {
    fn simple_write<T: ToBeBytes<N>, const N: usize>(&mut self, value: T) -> Result<()> {
        self.write_all(&value.to_be()).map_err(|e| e.into())
    }
}
