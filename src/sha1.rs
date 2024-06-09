pub struct Sha1 {
    h: [u32; 5],
    w: [u32; 80],
}

impl Sha1 {
    pub fn new() -> Sha1 {
        Sha1 {
            h: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
            w: [0; 80],
        }
    }

    pub fn finalize(self, ptr: *mut u8, mut len: usize, total: u64) -> [u32; 5] {
        let mut this = self;

        unsafe {
            if len < 56 {
                let total = (total * 8).to_be_bytes();

                *ptr.add(len) = 0x80;
                len += 1;

                while len < 56 {
                    *ptr.add(len) = 0x00;
                    len += 1;
                }

                for i in 0..total.len() {
                    *ptr.add(len) = total[i];
                    len += 1;
                }

                this = this.update(ptr, len);
            } else {
                *ptr.add(len) = 0x80;
                len += 1;

                while len < 64 {
                    *ptr.add(len) = 0x00;
                    len += 1;
                }

                this = this.update(ptr, len);
                len = 0;

                while len < 56 {
                    *ptr.add(len) = 0x00;
                    len += 1;
                }

                let total = (total * 8).to_be_bytes();

                for i in 0..total.len() {
                    *ptr.add(len) = total[i];
                    len += 1;
                }

                this = this.update(ptr, len);
            }
        }

        this.h
    }

    pub fn update(mut self, ptr: *const u8, len: usize) -> Self {
        fn rotate<const T: u32>(value: u32) -> u32 {
            (value << T) ^ (value >> (32 - T))
        }

        for i in 0..(len / 64) {
            let (mut a, mut b, mut c, mut d, mut e) = (self.h[0], self.h[1], self.h[2], self.h[3], self.h[4]);

            unsafe {
                for j in 0..16 {
                    let b0 = *ptr.add(i * 64 + j * 4 + 0) as u32;
                    let b1 = *ptr.add(i * 64 + j * 4 + 1) as u32;
                    let b2 = *ptr.add(i * 64 + j * 4 + 2) as u32;
                    let b3 = *ptr.add(i * 64 + j * 4 + 3) as u32;

                    self.w[j] = (b0 << 24) + (b1 << 16) + (b2 << 8) + b3;
                }

                for j in 16..80 {
                    self.w[j] = rotate::<1>(
                        self.w.get_unchecked(j - 3)
                            ^ self.w.get_unchecked(j - 8)
                            ^ self.w.get_unchecked(j - 14)
                            ^ self.w.get_unchecked(j - 16),
                    )
                }

                for j in 0..20 {
                    let f = ((b & c) | (!b & d)) + 0x5a827999;
                    let t = rotate::<5>(a) + f + e + self.w.get_unchecked(j);
                    e = d;
                    d = c;
                    c = rotate::<30>(b);
                    b = a;
                    a = t;
                }

                for j in 20..40 {
                    let f = (b ^ c ^ d) + 0x6ed9eba1;
                    let t = rotate::<5>(a) + f + e + self.w.get_unchecked(j);
                    e = d;
                    d = c;
                    c = rotate::<30>(b);
                    b = a;
                    a = t;
                }

                for j in 40..60 {
                    let f = ((b & c) | (b & d) | (c & d)) + 0x8f1bbcdc;
                    let t = rotate::<5>(a) + f + e + self.w.get_unchecked(j);
                    e = d;
                    d = c;
                    c = rotate::<30>(b);
                    b = a;
                    a = t;
                }

                for j in 60..80 {
                    let f = (b ^ c ^ d) + 0xca62c1d6;
                    let t = rotate::<5>(a) + f + e + self.w.get_unchecked(j);
                    e = d;
                    d = c;
                    c = rotate::<30>(b);
                    b = a;
                    a = t;
                }

                self.h[0] += a;
                self.h[1] += b;
                self.h[2] += c;
                self.h[3] += d;
                self.h[4] += e;
            }
        }

        self
    }
}
