use std::u8;

use da_boot_macros::Protocol;

use crate::err::Error;

#[derive(Default, Protocol)]
#[protocol(command = 0x01)]
pub(crate) struct Patch<'a> {
    #[protocol(tx)]
    addr: u32,
    #[protocol(tx)]
    len: u32,
    #[protocol(tx)]
    payload: &'a [u8],
}

#[derive(Default, Protocol)]
#[protocol(command = 0x02)]
pub(crate) struct DumpPreloader {
    #[protocol(rx)]
    size: u32,
    #[protocol(rx, getter, size = size)]
    preloader: Vec<u8>,
}

#[derive(Default, Protocol)]
#[protocol(command = 0x03)]
pub(crate) struct Return;
