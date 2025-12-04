use da_boot_macros::Protocol;

use crate::err::Error;

#[derive(Default, Protocol)]
#[protocol(command = 0xd8, echo)]
pub(crate) struct GetTargetConfig {
    #[protocol(rx)]
    config: u32,
    #[protocol(rx, status = 0)]
    status: u16,
}

impl GetTargetConfig {
    pub fn parse(&self) -> (bool, bool, bool) {
        (
            self.config & 0x1 != 0,
            self.config & 0x2 != 0,
            self.config & 0x4 != 0,
        )
    }
}

#[derive(Default, Protocol)]
#[protocol(command = 0xfd, echo)]
pub(crate) struct GetHwCode {
    #[protocol(rx, getter)]
    hwcode: u16,
    #[protocol(rx, status = 0)]
    status: u16,
}
