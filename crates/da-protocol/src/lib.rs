#![cfg_attr(not(feature = "std"), no_std)]

use core::{borrow::Borrow, fmt::Display};

use da_port::{SimpleRead, SimpleWrite};
use derive_ctor::ctor;
use derive_more::IsVariant;
use serde::{Deserialize, Serialize};

use crate::err::Error;

pub mod err;

pub type Result<T> = core::result::Result<T, Error>;

/// Protocol messages
#[derive(ctor, Serialize, Deserialize, IsVariant)]
#[repr(u8)]
pub enum Message<'a> {
    /// Heartbeat.
    Ack = 0x42,
    /// Read data at `addr` with `size` length.
    Read { addr: u32, size: u32 },
    /// Write `data` to `addr`.
    Write { addr: u32, data: &'a [u8] },
    /// Flush I and D-cache at `addr` with `size` aligned to 64.
    FlushCache { addr: u32, size: u32 },
    /// Jump to `addr`. The `addr` **must** contain **ARM** mode instructions.
    Jump { addr: u32 },
    /// Reset the device using watchdog.
    Reset,

    #[cfg(feature = "preloader")]
    /// Return to `usbdl_handler`.
    Return,
}

/// Protocol responses
#[derive(ctor, Serialize, Deserialize, IsVariant)]
#[repr(u8)]
pub enum Response<'a> {
    /// Operation succeed.
    Ack = 0xDD,
    /// Operation failed.
    Nack = !0xDD,
    /// Read data.
    Read { data: &'a [u8] },
}

/// `da-boot` protocol to communicate between host and device
///
/// The protocol itself is really simple:
/// - length of the payload - u32
/// - data
///
/// It's up to host to not overflow the buffer with `Message::Read`, `Message::Write` and `Response::Read`.
#[derive(ctor)]
pub struct Protocol<T: SimpleRead + SimpleWrite> {
    io: T,
}

impl<T: SimpleRead + SimpleWrite> Protocol<T> {
    /// Read data to the `buf` regardless of its' size.
    fn read_data<'a, U: serde::Deserialize<'a>>(&mut self, buf: &'a mut [u8]) -> Result<U> {
        let size = self.io.read_u32_be()?;
        self.io.read(&mut buf[..size as usize])?;
        let data = postcard::from_bytes(buf)?;

        Ok(data)
    }

    /// Write `data` to the target.
    ///
    /// The `buf` is used for serialization without allocating temporary buffer.
    fn write_data<'a, U: serde::Serialize + Borrow<U>>(
        &mut self,
        buf: &'a mut [u8],
        data: U,
    ) -> Result<()> {
        let bytes = postcard::to_slice(&data, buf)?;
        self.io.write_u32_be(bytes.len() as u32)?;
        self.io.write(&bytes).map_err(|e| e.into())
    }

    /// Receive message
    ///
    /// The message lives as long as the `buf` is valid.
    pub fn read_message<'a>(&mut self, buf: &'a mut [u8]) -> Result<Message<'a>> {
        self.read_data(buf)
    }

    /// Send message
    ///
    /// The `buf` is used to store the serialized data.
    pub fn send_message<'a, U: serde::Serialize + Borrow<Message<'a>>>(
        &mut self,
        buf: &mut [u8],
        message: U,
    ) -> Result<()> {
        self.write_data(buf, message)
    }

    /// Receive response
    ///
    /// The response lives as long as the `buf` is valid.
    pub fn read_response<'a>(&mut self, buf: &'a mut [u8]) -> Result<Response<'a>> {
        self.read_data(buf)
    }

    /// Send response
    ///
    /// The `buf` is used to store the serialized data.
    pub fn send_response<'a, U: serde::Serialize + Borrow<Response<'a>>>(
        &mut self,
        buf: &mut [u8],
        response: U,
    ) -> Result<()> {
        self.write_data(buf, response)
    }

    /// Recommended buffer size for read/write operations, considering preloader stack limitation
    #[must_use]
    pub const fn rw_buffer_size() -> usize {
        2048 - max(size_of::<Message>(), size_of::<Response>())
    }
}

impl Display for Message<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ack => write!(f, "ACK"),
            Self::Read { addr, size } => write!(f, "Read @ 0x{addr:08x} for 0x{size:x} bytes"),
            Self::Write { addr, data } => write!(f, "Write @ 0x{addr:08x}: {data:x?}"),
            Self::FlushCache { addr, size } => {
                write!(f, "Flush cache @ 0x{addr:08x} for 0x{size:x} bytes")
            }
            Self::Jump { addr } => write!(f, "Jump to 0x{addr:08x}"),
            Self::Reset => write!(f, "Reset"),

            #[cfg(feature = "preloader")]
            Self::Return => write!(f, "Jump to usbdl_handler"),
        }
    }
}

impl Display for Response<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ack => write!(f, "ACK"),
            Self::Nack => write!(f, "Not ACK"),
            Self::Read { data } => write!(f, "Data: {data:x?}"),
        }
    }
}

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}
