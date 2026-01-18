use crate::exploit::{Exploits, ExploitsDiscriminants};

pub mod da;

#[derive(Debug, Default)]
pub enum BootStage {
    #[default]
    BootROM,
    Preloader,
    DA1,
    DA2,
}

impl BootStage {
    pub fn trigger_exploits<'a>(&self) -> Option<ExploitsDiscriminants> {
        match self {
            Self::BootROM => None,
            Self::Preloader => None,
            // Select pumpkin because it doesn't corrupt bss
            Self::DA1 => Some(ExploitsDiscriminants::Pumpkin),
            Self::DA2 => Some(ExploitsDiscriminants::Croissant2),
        }
    }
}
