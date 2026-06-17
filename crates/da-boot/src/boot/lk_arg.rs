use acon::SoC;
use clap::ValueEnum;

use crate::boot::give_me_bytes_please;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "kebab_case")]
#[repr(u32)]
pub enum LkBootMode {
    #[default]
    Normal,
    Meta,
    Recovery,
    SwReboot,
    Factory,
    Advmeta,
    AteFactory,
    Alarm,
    Fastboot = 99,
    Download,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub enum ForbiddenMode {
    #[default]
    FactoryMode = 1,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct SecLimit {
    pub magic_num: u32,
    pub forbid_mode: ForbiddenMode,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct DAInfo6572 {
    pub addr: u32,
    pub arg1: u32,
    pub arg2: u32,
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct BootArgument6572 {
    magic: u32,
    mode: u32,
    e_flag: u32,
    log_port: u32,
    log_baudrate: u32,
    log_enable: u8,
    reserved: [u8; 3],
    dram_rank_num: u32,
    dram_rank_size: [u32; 4],
    boot_reason: u32,
    meta_com_type: u32,
    meta_com_id: u32,
    boot_time: u32,
    da_info: DAInfo6572,
    sec_limit: SecLimit,
}

impl BootArgument6572 {
    pub fn lk(mode: LkBootMode, dram_size_per_rank: u32, dram_ranks: u32) -> Self {
        let dram_rank_size = std::array::from_fn(|i| {
            if i < dram_ranks as usize {
                dram_size_per_rank
            } else {
                0
            }
        });
        Self {
            magic: 0x504c504c,
            mode: mode as u32,
            e_flag: 0,
            log_port: 0x11005000,
            log_baudrate: 921600,
            log_enable: 1,
            dram_rank_num: dram_ranks,
            dram_rank_size: dram_rank_size,
            boot_reason: 4,
            boot_time: 1337,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct Mblock {
    pub start: u64,
    pub size: u64,
    pub rank: u32,
    _pad: u32,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct MemDesc {
    pub start: u64,
    pub size: u64,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct DAInfo6595 {
    pub addr: u32,
    pub arg1: u32,
    pub arg2: u32,
    pub len: u32,
    pub sig_len: u32,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct VcoreDvfsInfo {
    pub pll_setting_num: u32,
    pub freq_setting_num: u32,
    pub low_freq_pll_setting_addr: u32,
    pub low_freq_cha_setting_addr: u32,
    pub low_freq_chb_setting_addr: u32,
    pub high_freq_pll_setting_addr: u32,
    pub high_freq_cha_setting_addr: u32,
    pub high_freq_chb_setting_addr: u32,
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct BootArgument6595 {
    pub magic: u32,
    pub mode: u32,
    pub e_flag: u32,
    pub log_port: u32,
    pub log_baudrate: u32,
    pub log_enable: u8,
    pub part_num: u8,
    _pad1: [u8; 2],
    pub dram_rank_num: u32,
    pub dram_rank_size: [u32; 4],
    _pad2: u32,

    pub mblock_num: u32,
    _pad3: u32,
    pub mblock: [Mblock; 4],

    pub orig_dram_num: u32,
    _pad4: u32,
    pub orig_dram_info: [MemDesc; 4],

    pub lca_reserved_mem: MemDesc,
    pub tee_reserved_mem: MemDesc,

    pub boot_reason: u32,
    pub meta_com_type: u32,
    pub meta_com_id: u32,
    pub boot_time: u32,

    pub da_info: DAInfo6595,
    pub sec_limit: SecLimit,

    pub part_info: u32,
    pub md_type: [u8; 4],
    pub ddr_reserve_enable: u32,
    pub ddr_reserve_success: u32,

    pub vcore_dvfs_info: VcoreDvfsInfo,

    pub dram_buf_size: u32,
    pub meta_uart_port: u32,
    pub smc_boot_opt: u32,
    pub lk_boot_opt: u32,
    pub kernel_boot_opt: u32,
    pub non_secure_sram_addr: u32,
    pub non_secure_sram_size: u32,
}

impl BootArgument6595 {
    pub fn lk(mode: LkBootMode, dram_size_per_rank: u32, dram_ranks: u32) -> Self {
        let dram_rank_size = std::array::from_fn(|i| {
            if i < dram_ranks as usize {
                dram_size_per_rank
            } else {
                0
            }
        });
        Self {
            magic: 0x504c504c,
            mode: mode as u32,
            e_flag: 0,
            log_port: 0x11002000,
            log_baudrate: 921600,
            log_enable: 1,
            dram_rank_num: dram_ranks,
            dram_rank_size: dram_rank_size,
            mblock_num: 2,
            mblock: [
                Mblock {
                    start: 0x40000000,
                    size: 0x40000000,
                    rank: 0,
                    ..Default::default()
                },
                Mblock {
                    start: 0x80000000,
                    size: 0x40000000,
                    rank: 1,
                    ..Default::default()
                },
                Mblock::default(),
                Mblock::default(),
            ],
            orig_dram_num: 2,
            orig_dram_info: [
                MemDesc {
                    start: 0x40000000,
                    size: 0x40000000,
                },
                MemDesc {
                    start: 0x80000000,
                    size: 0x40000000,
                },
                MemDesc::default(),
                MemDesc::default(),
            ],
            boot_reason: 4,
            boot_time: 1337,
            vcore_dvfs_info: VcoreDvfsInfo {
                pll_setting_num: 20,
                freq_setting_num: 51,
                low_freq_pll_setting_addr: 0x102AB8,
                low_freq_cha_setting_addr: 0x1029EC,
                low_freq_chb_setting_addr: 0x102B5C,
                high_freq_pll_setting_addr: 0x102B08,
                high_freq_cha_setting_addr: 0x102CF4,
                high_freq_chb_setting_addr: 0x102C28,
            },
            lk_boot_opt: 3,
            kernel_boot_opt: 3,
            non_secure_sram_addr: 0x10dc00,
            non_secure_sram_size: 0x2400,
            ..Default::default()
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum BootArgument {
    MT6572(BootArgument6572),
    MT6595(BootArgument6595),
}

impl BootArgument {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::MT6572(a) => give_me_bytes_please(a),
            Self::MT6595(a) => give_me_bytes_please(a),
        }
    }
}

pub fn get_for_soc(
    soc: SoC,
    mode: LkBootMode,
    dram_size_per_rank: u32,
    dram_ranks: u32,
) -> BootArgument {
    match soc {
        SoC::MT6572 => {
            BootArgument::MT6572(BootArgument6572::lk(mode, dram_size_per_rank, dram_ranks))
        }
        SoC::MT6595 => {
            BootArgument::MT6595(BootArgument6595::lk(mode, dram_size_per_rank, dram_ranks))
        }
        _ => unreachable!(),
    }
}
