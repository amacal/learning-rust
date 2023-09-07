pub struct Adler32 {
    a: u16,
    b: u16,
}

impl Adler32 {
    pub fn new() -> Self {
        Self { a: 1, b: 0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        for &byte in data.iter() {
            self.a = ((self.a as u32 + byte as u32) % 65521) as u16;
            self.b = ((self.b as u32 + self.a as u32) % 65521) as u16;
        }
    }

    pub fn finalize(&self) -> u32 {
        ((self.b as u32) << 16) + (self.a as u32)
    }
}
