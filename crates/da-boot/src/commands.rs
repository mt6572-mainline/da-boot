use da_boot_macros::Protocol;

use crate::err::Error;

/// A command to upload Download Agent to the device
///
/// Once the preloader patcher was ran, the `addr` field is handled properly, otherwise preloader overwrites it with the `DA_ADDR` constant
#[derive(Default, Protocol)]
#[protocol(command = 0xd7)]
pub(crate) struct SendDA<'a> {
    /// DA address
    #[protocol(echo)]
    addr: u32,
    /// DA length
    #[protocol(echo)]
    payload_len: u32,
    /// DA signature length
    #[protocol(echo)]
    da_sig_len: u32,
    /// Status for DA range and overlap (no-op for the mt6572)
    #[protocol(rx, status = 0)]
    zero_status: u16,
    /// DA
    #[protocol(tx)]
    payload: &'a [u8],
    /// DA checksum
    #[protocol(rx)]
    checksum: u16,
    /// usbdl_verify_da status
    #[protocol(rx, status = 0)]
    status: u16,
}

/// A command to jump to previously uploaded Download Agent
#[derive(Default, Protocol)]
#[protocol(command = 0xd5)]
pub(crate) struct JumpDA {
    /// DA address
    #[protocol(echo)]
    addr: u32,
    /// DA jump status
    #[protocol(rx, status = 0)]
    status: u16,
}

/// A command to read u32 from the memory
#[derive(Default, Protocol)]
#[protocol(command = 0xd1)]
pub(crate) struct Read32 {
    /// Start address
    #[protocol(echo)]
    addr: u32,
    /// Number of u32 to read
    #[protocol(echo)]
    dwords: u32,
    /// Status after sec_region_check (no-op for the mt6572)
    #[protocol(rx, status = 0)]
    status: u16,
    /// U32s
    #[protocol(rx, size = dwords)]
    pub buf: Vec<u32>,
    /// Read status (no-op for the mt6572)
    #[protocol(rx, status = 0)]
    final_status: u16,
}
