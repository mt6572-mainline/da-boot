#![cfg_attr(not(feature = "std"), no_std)]
#![feature(const_trait_impl, const_default, const_cmp)]

use core::ops::Range;
use core::{borrow::Borrow, fmt::Display};

use derive_ctor::ctor;
use derive_more::IsVariant;
use serde::{Deserialize, Serialize};
use simpleport::{SimpleRead, SimpleWrite};

use crate::err::Error;

pub mod err;

#[derive(Serialize, Deserialize)]
pub enum HookId {
    /// Allow booting boot.img or recovery.img from the RAM
    MtPartGenericRead,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct PreloaderRunnerParams {
    /// `bldr_jump` function pointer (for call)
    pub ptr_bldr_jump: u32,
}

const impl Default for PreloaderRunnerParams {
    fn default() -> Self {
        Self { ptr_bldr_jump: 0 }
    }
}

impl PreloaderRunnerParams {
    pub fn new(ptr_bldr_jump: u32) -> Self {
        Self { ptr_bldr_jump }
    }

    pub fn is_valid(&self) -> bool {
        self.ptr_bldr_jump != 0
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct LKRunnerParams {
    /// `mt_part_generic_read` function pointer (for hook)
    pub ptr_mt_part_generic_read: u32,
    /// `mt_part_get_partition` function pointer (for call)
    pub ptr_mt_part_get_partition: u32,
    /// Address for the boot.img
    pub bootimg_scratch_addr: u32,
}

const impl Default for LKRunnerParams {
    fn default() -> Self {
        Self {
            ptr_mt_part_generic_read: 0,
            ptr_mt_part_get_partition: 0,
            bootimg_scratch_addr: 0,
        }
    }
}

impl LKRunnerParams {
    pub fn new(
        ptr_mt_part_generic_read: u32,
        ptr_mt_part_get_partition: u32,
        bootimg_scratch_addr: u32,
    ) -> Self {
        Self {
            ptr_mt_part_generic_read,
            ptr_mt_part_get_partition,
            bootimg_scratch_addr,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.ptr_mt_part_generic_read != 0
            && self.ptr_mt_part_get_partition != 0
            && self.bootimg_scratch_addr != 0
    }
}

#[derive(Serialize, Deserialize)]
pub enum ParamsType {
    /// Preloader params
    Preloader(PreloaderRunnerParams),
    /// LK params
    LK(LKRunnerParams),
}

/// Protocol messages
#[derive(ctor, Serialize, Deserialize, IsVariant)]
#[repr(u8)]
pub enum Message {
    /// Heartbeat.
    Ack = 0xA0,
    /// Read data at `addr` with `size` length.
    Read { addr: u32, size: u32 },
    /// Write `data` to `addr`.
    Write { addr: u32, size: u32 },
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
    /// Get free memory range with `size`
    GetFreeRange { size: u32 },
    /// Forbid download to the given range
    BlacklistRange(Range<u32>),
    /// Set params with a given type
    SetParams(ParamsType),
}

#[derive(Debug, Serialize, Deserialize, IsVariant)]
pub enum ProtocolError {
    /// Command is not supported
    NotSupported,
    /// This shouldn't have happened
    Unreachable,
    /// Download is forbidden due to memory range blacklist
    DownloadForbidden,
    /// Parameters are not valid.
    InvalidParams,
}

/// Protocol responses
#[derive(Debug, ctor, Serialize, Deserialize, IsVariant)]
#[repr(u8)]
pub enum Response {
    /// Operation succeed.
    Ack = 0xDA,
    /// Operation failed.
    Nack(ProtocolError),
    /// Range address.
    Range(Option<u32>),
}

const BUF_SIZE: usize = size_of::<Message>().max(size_of::<Response>());

/// `da-boot` protocol to communicate between host and device
///
/// The protocol itself is really simple:
/// - length of the payload - u32
/// - data
pub struct Protocol<T: SimpleRead + SimpleWrite> {
    pub io: T,
    buf: [u8; BUF_SIZE],
}

impl<T: SimpleRead + SimpleWrite> Protocol<T> {
    pub fn new(io: T) -> Self {
        Self {
            io,
            buf: [0; BUF_SIZE],
        }
    }

    /// Read data to the `buf` regardless of its' size.
    fn read_data<'a, U: serde::Deserialize<'a>>(
        &'a mut self,
    ) -> Result<U, Error<<T as SimpleRead>::Error>> {
        let size = self.io.read_u32_be().map_err(Error::Transport)?;

        self.io
            .read(&mut self.buf[..size as usize])
            .map_err(Error::Transport)?;
        let data = postcard::from_bytes(&self.buf)?;

        Ok(data)
    }

    /// Write `data` to the target.
    ///
    /// The `buf` is used for serialization without allocating temporary buffer.
    fn write_data<'a, U: serde::Serialize + Borrow<U>>(
        &mut self,
        data: U,
    ) -> Result<(), Error<<T as SimpleWrite>::Error>> {
        let bytes = postcard::to_slice(&data, &mut self.buf)?;
        self.io
            .write_u32_be(bytes.len() as u32)
            .map_err(Error::Transport)?;
        self.io.write(&bytes).map_err(Error::Transport)
    }

    /// Receive message
    ///
    /// The message lives as long as the `buf` is valid.
    pub fn read_message(&mut self) -> Result<Message, Error<<T as SimpleRead>::Error>> {
        self.read_data()
    }

    /// Send message
    ///
    /// The `buf` is used to store the serialized data.
    pub fn send_message<U: serde::Serialize + Borrow<Message>>(
        &mut self,
        message: U,
    ) -> Result<(), Error<<T as SimpleWrite>::Error>> {
        self.write_data(message)
    }

    /// Receive response
    ///
    /// The response lives as long as the `buf` is valid.
    pub fn read_response(&mut self) -> Result<Response, Error<<T as SimpleRead>::Error>> {
        self.read_data()
    }

    /// Send response
    ///
    /// The `buf` is used to store the serialized data.
    pub fn send_response<U: serde::Serialize + Borrow<Response>>(
        &mut self,
        response: U,
    ) -> Result<(), Error<<T as SimpleWrite>::Error>> {
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

impl Display for ParamsType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Preloader(_) => write!(f, "Preloader"),
            Self::LK(_) => write!(f, "LK"),
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ack => write!(f, "ACK"),
            Self::Read { addr, size } => write!(f, "Read {size:#x} bytes 0x{addr:#10x}"),
            Self::Write { addr, size } => {
                write!(f, "Write {size:#x} bytes at {addr:#10x}")
            }
            Self::FlushCache { addr, size } => {
                write!(f, "Flush cache @ {addr:#10x} for {size:#x} bytes")
            }
            Self::Jump { addr, r0, r1 } => {
                write!(f, "Jump to {addr:#10x}")?;
                if let Some(r0) = r0 {
                    write!(f, " R0: {r0:#10x}")?;
                }
                if let Some(r1) = r1 {
                    write!(f, " R1: {r1:#10x}")?;
                }
                Ok(())
            }
            Self::Reset => write!(f, "Reset"),
            Self::Hook(hook) => write!(f, "Hook: {hook}"),
            Self::GetFreeRange { size } => write!(f, "Get free range with {size:#x} bytes"),
            Self::BlacklistRange(range) => {
                write!(f, "Blacklist range {:#x}..{:#x}", range.start, range.end)
            }
            Self::SetParams(params) => write!(f, "Set params for the {params}"),
        }
    }
}

impl Display for ProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotSupported => write!(f, "Not supported"),
            Self::Unreachable => write!(f, "Unreachable"),
            Self::DownloadForbidden => write!(f, "Download forbidden"),
            Self::InvalidParams => write!(f, "Invalid parameters"),
        }
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ack => write!(f, "ACK"),
            Self::Nack(e) => write!(f, "Not ACK: {e}"),
            Self::Range(maybe_addr) => {
                if let Some(addr) = maybe_addr {
                    write!(f, "Range at {addr:#10x}")
                } else {
                    write!(f, "Free range list is exhaustd")
                }
            }
        }
    }
}
