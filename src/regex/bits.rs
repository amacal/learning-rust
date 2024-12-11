pub struct Bits([u64; 4]);

impl Bits {
    pub fn new() -> Self {
        Self([0; 4])
    }

    pub fn get(&self, idx: u8) -> bool {
        self.0[idx as usize / 64] & ((1 as u64) << (idx as u64 % 64)) > 0
    }

    pub fn set(&mut self, idx: u8) {
        self.0[idx as usize / 64] |= (1 as u64) << (idx as u64 % 64);
    }
}
