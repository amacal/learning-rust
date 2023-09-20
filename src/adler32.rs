pub struct Adler32 {
    a: u32,
    b: u32,
    i: u32,
}

impl Adler32 {
    pub fn new() -> Self {
        Self { a: 1, b: 0, i: 0 }
    }

    pub fn update(&mut self, data: &[u8]) {
        let step = 4096;

        for &byte in data.iter() {
            self.a = self.a + byte as u32;
            self.b = self.b + self.a;

            if self.i % step == 0 {
                self.a %= 65521;
                self.b %= 65521;
            }

            self.i += 1;
        }
    }

    pub fn finalize(&self) -> u32 {
        ((self.b % 65521) << 16) + (self.a % 65521)
    }
}
