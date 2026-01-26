#![cfg_attr(not(feature = "std"), no_std)]

use core::{borrow::Borrow, fmt::Display};

use derive_ctor::ctor;
use derive_more::IsVariant;
use serde::{Deserialize, Serialize};
use simpleport::{SimpleRead, SimpleWrite};

use crate::err::Error;

pub mod err;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Serialize, Deserialize)]
pub enum HookId {
    /// Allow booting boot.img or recovery.img from the RAM
    MtPartGenericRead,
}

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
    Jump {
        addr: u32,
        r0: Option<u32>,
        r1: Option<u32>,
    },
    /// Reset the device using watchdog.
    Reset,
    /// Setup hook
    Hook(HookId),

    /// Return to `usbdl_handler` in the preloader mode.
    Return,
}

#[cfg(not(feature = "std"))]
impl Message<'_> {
    pub fn debug(&self) -> u8 {
        match self {
            Self::Ack => b'A',
            Self::Read { .. } => b'R',
            Self::Write { .. } => b'W',
            Self::FlushCache { .. } => b'F',
            Self::Jump { .. } => b'J',
            Self::Reset => b'W',
            Self::Hook(_) => b'H',
            Self::Return => b'P',
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum NotFoundError {
    BldrJump,
    MtPartGenericRead,
    UsbDlHandler,
}

#[derive(Serialize, Deserialize, IsVariant)]
pub enum ProtocolError {
    /// Command is not supported
    NotSupported,
    /// Didn't find any occurences of the match
    NotFound(NotFoundError),
    /// The control flow reached the point where it shouldn't be
    Unreachable,
}

#[cfg(not(feature = "std"))]
impl ProtocolError {
    pub fn debug(&self) -> u8 {
        match self {
            Self::NotSupported => b'N',
            Self::NotFound(_) => b'F',
            Self::Unreachable => b'U',
        }
    }
}

/// Protocol responses
#[derive(ctor, Serialize, Deserialize, IsVariant)]
#[repr(u8)]
pub enum Response<'a> {
    /// Operation succeed.
    Ack = 0xDD,
    /// Operation failed.
    Nack(ProtocolError),
    /// Read data.
    Read { data: &'a [u8] },
    /// Value.
    Value(u32),
}

#[cfg(not(feature = "std"))]
impl Response<'_> {
    pub fn debug(&self) -> u8 {
        match self {
            Self::Ack => b'A',
            Self::Nack(_) => b'N',
            Self::Read { .. } => b'R',
            Self::Value(_) => b'V',
        }
    }
}

/// `da-boot` protocol to communicate between host and device
///
/// The protocol itself is really simple:
/// - length of the payload - u32
/// - data
///
/// It's up to host to not overflow the buffer with `Message::Read`, `Message::Write` and `Response::Read`.
#[derive(ctor)]
pub struct Protocol<T: SimpleRead + SimpleWrite, const N: usize> {
    io: T,
    buf: [u8; N],
}

impl<T: SimpleRead + SimpleWrite, const N: usize> Protocol<T, N> {
    /// Recommended buffer size for read/write operations, considering preloader stack limitation.
    pub const RW_BUFFER_SIZE: usize = 2048 - max(size_of::<Message>(), size_of::<Response>());

    /// Read data to the `buf` regardless of its' size.
    fn read_data<'a, U: serde::Deserialize<'a>>(&'a mut self) -> Result<U> {
        let size = self.io.read_u32_be()?;
        self.io.read(&mut self.buf[..size as usize])?;
        let data = postcard::from_bytes(&self.buf)?;

        Ok(data)
    }

    /// Write `data` to the target.
    ///
    /// The `buf` is used for serialization without allocating temporary buffer.
    fn write_data<'a, U: serde::Serialize + Borrow<U>>(&mut self, data: U) -> Result<()> {
        let bytes = postcard::to_slice(&data, &mut self.buf)?;
        self.io.write_u32_be(bytes.len() as u32)?;
        self.io.write(&bytes).map_err(|e| e.into())
    }

    /// Receive message
    ///
    /// The message lives as long as the `buf` is valid.
    pub fn read_message(&mut self) -> Result<Message<'_>> {
        self.read_data()
    }

    /// Send message
    ///
    /// The `buf` is used to store the serialized data.
    pub fn send_message<'a, U: serde::Serialize + Borrow<Message<'a>>>(
        &mut self,
        message: U,
    ) -> Result<()> {
        self.write_data(message)
    }

    /// Receive response
    ///
    /// The response lives as long as the `buf` is valid.
    pub fn read_response(&mut self) -> Result<Response<'_>> {
        self.read_data()
    }

    /// Send response
    ///
    /// The `buf` is used to store the serialized data.
    pub fn send_response<'a, U: serde::Serialize + Borrow<Response<'a>>>(
        &mut self,
        response: U,
    ) -> Result<()> {
        self.write_data(response)
    }
}

impl Display for HookId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MtPartGenericRead => write!(f, "mt_part_generic_read"),
        }
    }
}

impl Display for Message<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ack => write!(f, "ACK"),
            Self::Read { addr, size } => write!(f, "Read @ 0x{addr:08x} for 0x{size:x} bytes"),
            Self::Write { addr, data } => {
                write!(f, "Write @ 0x{addr:08x}: [")?;
                format_slice(f, data)?;
                write!(f, "]")
            }
            Self::FlushCache { addr, size } => {
                write!(f, "Flush cache @ 0x{addr:08x} for 0x{size:x} bytes")
            }
            Self::Jump { addr, r0, r1 } => {
                write!(f, "Jump to 0x{addr:08x}")?;
                if let Some(r0) = r0 {
                    write!(f, " R0: 0x{r0:08x}")?;
                }
                if let Some(r1) = r1 {
                    write!(f, " R1: 0x{r1:08x}")?;
                }
                Ok(())
            }
            Self::Reset => write!(f, "Reset"),
            Self::Hook(hook) => write!(f, "Hook: {hook}"),
            Self::Return => write!(f, "Jump to usbdl_handler"),
        }
    }
}

impl Display for NotFoundError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BldrJump => write!(f, "bldr_jump"),
            Self::MtPartGenericRead => write!(f, "mt_part_generic_read"),
            Self::UsbDlHandler => write!(f, "usbdl_handler"),
        }
    }
}

impl Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotSupported => write!(f, "Not supported"),
            Self::NotFound(error) => write!(f, "Not found: {error}"),
            Self::Unreachable => write!(f, "Unreachable"),
        }
    }
}

impl Display for Response<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ack => write!(f, "ACK"),
            Self::Nack(e) => write!(f, "Not ACK: {e}"),
            Self::Read { data } => {
                write!(f, "Data: [")?;
                format_slice(f, data)?;
                write!(f, "]")
            }
            Self::Value(value) => write!(f, "Value: {value}"),
        }
    }
}

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

fn format_slice(f: &mut core::fmt::Formatter, data: &[u8]) -> core::fmt::Result {
    for (i, byte) in data.iter().enumerate() {
        if i != 0 {
            write!(f, ", ")?;
        }
        write!(f, "{:#04x}", byte)?;
    }

    Ok(())
}
