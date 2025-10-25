use da_boot_macros::Protocol;

use crate::err::Error;

#[derive(Default, Protocol)]
pub(crate) struct Sync {
    #[protocol(tx)]
    ack: u32,
    #[protocol(rx, status = 0x1337)]
    status: u16,
}

#[derive(Default, Protocol)]
pub(crate) struct RunPayload<'a> {
    #[protocol(tx)]
    addr: u32,
    #[protocol(tx)]
    payload_len: u32,
    #[protocol(tx)]
    payload: &'a [u8],
    #[protocol(rx, status = 0)]
    status: u16,
}
