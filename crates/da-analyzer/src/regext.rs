use yaxpeax_arm::armv7::Reg;

pub trait RegExt {
    fn is_lr(&self) -> bool;
    fn is_pc(&self) -> bool;
}

pub trait RegListExt {
    fn has_lr(&self) -> bool;
    fn has_pc(&self) -> bool;
}

impl RegExt for Reg {
    fn is_lr(&self) -> bool {
        self.number() == 14
    }

    fn is_pc(&self) -> bool {
        self.number() == 15
    }
}

impl RegListExt for u16 {
    fn has_lr(&self) -> bool {
        (self & (1 << 14)) != 0
    }

    fn has_pc(&self) -> bool {
        (self & (1 << 15)) != 0
    }
}
