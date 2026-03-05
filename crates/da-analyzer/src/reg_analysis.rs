#[derive(Debug, Default, PartialEq, Eq)]
pub enum RegState {
    #[default]
    Unknown,
    Const(u32),
}

#[derive(Debug, Default)]
pub struct RegWriteTracker {
    regs: [RegState; 16],
}

impl RegWriteTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn call(&mut self) {
        for i in 0..=3 {
            self.regs[i] = RegState::Unknown;
        }
    }

    pub fn store(&mut self, reg: u8, value: u32) {
        self.regs[reg as usize] = RegState::Const(value);
    }

    pub fn get(&self, reg: u8) -> Option<u32> {
        match self.regs[reg as usize] {
            RegState::Unknown => None,
            RegState::Const(v) => Some(v),
        }
    }
}
