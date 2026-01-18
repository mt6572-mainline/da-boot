use std::u8;

use da_boot_macros::Protocol;

use crate::err::Error;

#[derive(Default, Protocol)]
#[protocol(naked)]
pub(crate) struct DA1Setup {
    /// Sync byte, always 0xc0
    ///
    /// Found by `non-SYNC_CHAR(0x%02X) received from DA` string
    #[protocol(rx, status = 0xc0)]
    sync: u8,

    /// NAND flash status
    ///
    /// Found by `Get the nand ret (0x%02X)` string
    #[protocol(rx)]
    nand_ret: u32,
    /// Found by `Receive NAND ID Count=%d` string
    #[protocol(rx)]
    nand_id_count: u16,
    /// Found by `Receive Nand ID=0x%x` string
    #[protocol(rx)]
    _nand_id1: u16,
    /// Found by `Receive Nand ID=0x%x` string
    #[protocol(rx)]
    _nand_id2: u16,
    /// Found by `Receive Nand ID=0x%x` string
    #[protocol(rx)]
    _nand_id3: u16,
    /// Found by `Receive Nand ID=0x%x` string
    #[protocol(rx)]
    _nand_id4: u16,

    /// eMMC flash status
    ///
    /// Found by `Get the emmc ret (0x%02X)` string
    #[protocol(rx)]
    emmc_ret: u32,
    /// Found by `Get the emmc id (0x%02X,0x%02X,0x%02X,0x%02X)` string
    #[protocol(rx)]
    _emmc_id1: u32,
    /// Found by `Get the emmc id (0x%02X,0x%02X,0x%02X,0x%02X)` string
    #[protocol(rx)]
    _emmc_id2: u32,
    /// Found by `Get the emmc id (0x%02X,0x%02X,0x%02X,0x%02X)` string
    #[protocol(rx)]
    _emmc_id3: u32,
    /// Found by `Get the emmc id (0x%02X,0x%02X,0x%02X,0x%02X)` string
    #[protocol(rx)]
    _emmc_id4: u32,

    /// DA seems to ignore it
    #[protocol(tx, always = 0x0)]
    _cont: u8,

    /// DA minor version
    ///
    /// Found by `DA_v%u.%u` string
    #[protocol(rx)]
    minor: u8,
    /// DA major version
    ///
    /// Found by `DA_v%u.%u` string
    #[protocol(rx)]
    major: u8,
    /// Likely something related to the baseband
    #[protocol(rx)]
    _unknown: u8,

    /// BootROM version
    ///
    /// Found by `BROM Version: %d(0x%02X)` string
    #[protocol(tx, always = 0x1)]
    brom_version: u8,
    /// Preloader version
    ///
    /// Found by `BLOADER Version: %d(0x%02X)` string
    #[protocol(tx, always = 0x1)]
    preloader_version: u8,
    /// Found by `NOR_CFG: m_nor_chip_select[0]=\"%s\"(0x%02X)` string
    #[protocol(tx, always = 0x0)]
    _nor_chip_select1: u8,
    /// Found by `NOR_CFG: m_nor_chip_select[1]=\"%s\"(0x%02X)` string
    #[protocol(tx, always = 0x0)]
    _nor_chip_select2: u8,
    /// Found by `NAND_CFG: m_nand_chip_select=\"%s\"(0x%02X)` string
    #[protocol(tx, always = 0x0)]
    _nand_chip_select: u8,
    /// Found by `NAND_CFG: m_nand_acccon(0x%08X)` string
    #[protocol(tx, always = 0x0)]
    _nand_acccon: u32,

    /// Found by `SyncBmtInfoWithDA` function
    #[protocol(tx, always = 0x0)]
    bmt_present: u8,
    /// Found by `SyncBmtInfoWithDA` function
    #[protocol(tx, always = 0x0)]
    bmt_size: u32,
    /// Found by `force_charge(%d)` string
    ///
    /// Possible values:
    /// - 0 - device with battery
    /// - 1 - device without battery
    /// - 2 - auto
    #[protocol(tx, always = 0x2)]
    charge_mode: u8,
    /// Found by `reset_key(%d)` string
    #[protocol(tx, always = 0x52)]
    reset_mode: u8,
    /// Found by `EXT_CLOCK: ext_clock(0x%02X)` string
    ///
    /// Possible values:
    /// - 1 - 13MHz
    /// - 2 - 26MHz (used by most SoCs)
    /// - 3 - 39MHz
    /// - 4 - 52MHz
    /// - 254 - auto
    /// - 255 - unknown
    #[protocol(tx, always = 0x2)]
    external_clock_freq: u8,
    /// Found by `MSDC_BOOT_CH: channel(0x%02X), 0 - default` string
    ///
    /// 0 is default value
    #[protocol(tx, always = 0x0)]
    msdc_channel: u8,
    /// DRAM status
    ///
    /// 0 if already initialized
    #[protocol(rx, status = 0x0)]
    dram_status: u32,
}

impl DA1Setup {
    pub fn major(&self) -> u8 {
        self.major
    }

    pub fn minor(&self) -> u8 {
        self.minor
    }
}

#[derive(Default, Protocol)]
#[protocol(command = 0x7b)]
pub(crate) struct Write32 {
    #[protocol(tx)]
    addr: u32,
    #[protocol(tx)]
    data: u32,
    #[protocol(rx, status = 0x5a)]
    ack: u8,
}
