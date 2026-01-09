#![cfg_attr(not(feature = "std"), no_std)]

use core::{borrow::Borrow, fmt::Display};

use derive_ctor::ctor;
use derive_more::IsVariant;
use serde::{Deserialize, Serialize};
use simpleport::{SimpleRead, SimpleWrite};
use ufmt::{uDisplay, uWrite, uwrite};

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
    Jump {
        addr: u32,
        r1: Option<u32>,
        r2: Option<u32>,
    },
    /// Reset the device using watchdog.
    Reset,

    /// Return to `usbdl_handler` in the preloader mode.
    Return,
}

#[derive(ctor, Serialize, Deserialize, IsVariant)]
pub enum ProtocolError {
    /// Command is not supported
    NotSupported,
    /// The control flow reached the point where it shouldn't be
    Unreachable,
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

impl uDisplay for Message<'_> {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> core::result::Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        match self {
            Self::Ack => uwrite!(f, "ACK"),
            Self::Read { addr, size } => {
                uwrite!(f, "Read @ 0x{:08x} for 0x{:x} bytes", *addr, *size)
            }
            Self::Write { addr, data } => {
                uwrite!(f, "Write @ 0x{:08x}: [", *addr)?;
                format_slice(f, data)?;
                uwrite!(f, "]")
            }
            Self::FlushCache { addr, size } => {
                uwrite!(f, "Flush cache @ 0x{:08x} for 0x{:x} bytes", *addr, *size)
            }
            Self::Jump { addr, r1, r2 } => {
                uwrite!(f, "Jump to 0x{:08x}", *addr)?;
                if let Some(r1) = r1 {
                    uwrite!(f, " R1: 0x{:08x}", *r1)?;
                }
                if let Some(r2) = r2 {
                    uwrite!(f, " R2: 0x{:08x}", *r2)?;
                }
                Ok(())
            }
            Self::Reset => uwrite!(f, "Reset"),

            Self::Return => uwrite!(f, "Jump to usbdl_handler"),
        }
    }
}

impl uDisplay for ProtocolError {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> core::result::Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        match self {
            Self::NotSupported => uwrite!(f, "Not supported"),
            Self::Unreachable => uwrite!(f, "Unreachable"),
        }
    }
}

impl uDisplay for Response<'_> {
    fn fmt<W>(&self, f: &mut ufmt::Formatter<'_, W>) -> core::result::Result<(), W::Error>
    where
        W: ufmt::uWrite + ?Sized,
    {
        match self {
            Self::Ack => uwrite!(f, "ACK"),
            Self::Nack(e) => uwrite!(f, "Not ACK: {}", e),
            Self::Read { data } => {
                uwrite!(f, "Data: [")?;
                format_slice(f, data)?;
                uwrite!(f, "]")
            }
        }
    }
}

#[cfg(feature = "std")]
struct Adapter<'a, 'b>(&'a mut core::fmt::Formatter<'b>);

#[cfg(feature = "std")]
impl<'a, 'b> uWrite for Adapter<'a, 'b> {
    type Error = core::fmt::Error;

    fn write_str(&mut self, s: &str) -> core::result::Result<(), Self::Error> {
        core::fmt::Write::write_str(self.0, s)
    }
}

#[cfg(feature = "std")]
impl Display for Message<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut adapter = Adapter(f);
        uwrite!(&mut adapter, "{}", self)
    }
}

#[cfg(feature = "std")]
impl Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut adapter = Adapter(f);
        uwrite!(&mut adapter, "{}", self)
    }
}

#[cfg(feature = "std")]
impl Display for Response<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut adapter = Adapter(f);
        uwrite!(&mut adapter, "{}", self)
    }
}

const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

fn format_slice<W: ufmt::uWrite + ?Sized>(
    f: &mut ufmt::Formatter<'_, W>,
    data: &[u8],
) -> core::result::Result<(), W::Error> {
    for (i, byte) in data.iter().enumerate() {
        if i != 0 {
            uwrite!(f, ", ")?;
        }
        uwrite!(f, "{:#04x}", *byte)?;
    }

    Ok(())
}
