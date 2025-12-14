#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
use serialport::SerialPort;

use crate::err::Error;

pub mod err;

pub type Result<T> = core::result::Result<T, Error>;

#[cfg(feature = "std")]
pub type Port = Box<dyn SerialPort>;

pub trait FromBytes<const N: usize> {
    fn from_be(bytes: [u8; N]) -> Self;
    fn from_le(bytes: [u8; N]) -> Self;
}

pub trait ToBytes<const N: usize> {
    fn to_be(&self) -> [u8; N];
    fn to_le(&self) -> [u8; N];
}

pub trait SimpleRead {
    #[cfg(not(feature = "std"))]
    fn read(&mut self, buf: &mut [u8], n: usize) -> Result<()>;
    #[cfg(feature = "std")]
    fn read(&mut self, buf: &mut [u8]) -> Result<()>;

    fn simple_read_be<T: FromBytes<N>, const N: usize>(&mut self) -> Result<T> {
        let mut bytes = [0; N];
        #[cfg(not(feature = "std"))]
        Self::read(self, &mut bytes, N);
        #[cfg(feature = "std")]
        Self::read(self, &mut bytes)?;
        Ok(T::from_be(bytes))
    }

    fn simple_read_le<T: FromBytes<N>, const N: usize>(&mut self) -> Result<T> {
        let mut bytes = [0; N];
        #[cfg(not(feature = "std"))]
        Self::read(self, &mut bytes, N)?;
        #[cfg(feature = "std")]
        Self::read(self, &mut bytes)?;
        Ok(T::from_le(bytes))
    }

    fn read_u8(&mut self) -> Result<u8> {
        self.simple_read_be()
    }

    fn read_u16_be(&mut self) -> Result<u16> {
        self.simple_read_be()
    }

    fn read_u32_be(&mut self) -> Result<u32> {
        self.simple_read_be()
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        self.simple_read_le()
    }

    fn read_u32_le(&mut self) -> Result<u32> {
        self.simple_read_le()
    }
}

pub trait SimpleWrite {
    fn write(&mut self, buf: &[u8]) -> Result<()>;

    fn simple_write_be<T: ToBytes<N>, const N: usize>(&mut self, value: T) -> Result<()> {
        Self::write(self, &value.to_be()).map_err(|e| e.into())
    }

    fn simple_write_le<T: ToBytes<N>, const N: usize>(&mut self, value: T) -> Result<()> {
        Self::write(self, &value.to_le()).map_err(|e| e.into())
    }

    fn write_u8(&mut self, value: u8) -> Result<()> {
        self.simple_write_be(value)
    }

    fn write_u16_be(&mut self, value: u16) -> Result<()> {
        self.simple_write_be(value)
    }

    fn write_u32_be(&mut self, value: u32) -> Result<()> {
        self.simple_write_be(value)
    }

    fn write_u16_le(&mut self, value: u16) -> Result<()> {
        self.simple_write_le(value)
    }

    fn write_u32_le(&mut self, value: u32) -> Result<()> {
        self.simple_write_le(value)
    }
}

impl FromBytes<1> for u8 {
    fn from_be(bytes: [u8; 1]) -> Self {
        Self::from_be_bytes(bytes)
    }

    fn from_le(bytes: [u8; 1]) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl ToBytes<1> for u8 {
    fn to_be(&self) -> [u8; 1] {
        self.to_be_bytes()
    }

    fn to_le(&self) -> [u8; 1] {
        self.to_le_bytes()
    }
}

impl FromBytes<2> for u16 {
    fn from_be(bytes: [u8; 2]) -> Self {
        Self::from_be_bytes(bytes)
    }

    fn from_le(bytes: [u8; 2]) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl ToBytes<2> for u16 {
    fn to_be(&self) -> [u8; 2] {
        self.to_be_bytes()
    }

    fn to_le(&self) -> [u8; 2] {
        self.to_le_bytes()
    }
}

impl FromBytes<4> for u32 {
    fn from_be(bytes: [u8; 4]) -> Self {
        Self::from_be_bytes(bytes)
    }

    fn from_le(bytes: [u8; 4]) -> Self {
        Self::from_le_bytes(bytes)
    }
}

impl ToBytes<4> for u32 {
    fn to_be(&self) -> [u8; 4] {
        self.to_be_bytes()
    }

    fn to_le(&self) -> [u8; 4] {
        self.to_le_bytes()
    }
}

#[cfg(feature = "std")]
impl SimpleRead for Port {
    fn read(&mut self, buf: &mut [u8]) -> Result<()> {
        self.read_exact(buf).map_err(|e| e.into())
    }
}

#[cfg(feature = "std")]
impl SimpleWrite for Port {
    fn write(&mut self, buf: &[u8]) -> Result<()> {
        self.write_all(buf).map_err(|e| e.into())
    }
}
