use ::core::arch;

// ABCD         xmm0
// E0		    xmm1
// E1		    xmm2
// MSG0		    xmm3
// MSG1		    xmm4
// MSG2		    xmm5
// MSG3		    xmm6
// SHUF_MASK	xmm7

arch::global_asm!(
    "
    .global _sha1_update;

    _sha1_update:
        # rounds 0..3
        movdqa xmm3, [rsi + 0]    ; MSG0 = ptr[0..3]
        pshufb xmm3, xmm7         ; MSG0 endianness
        paddd xmm1, xmm3          ; E0 = E0 + MSG0
        movdqa xmm2, xmm0         ; E2 = ABCD
        sha1rnds4 xmm0, xmm1, 0   ; round 0..19

        # rounds 4..7
        movdqa xmm4, [rsi + 16];
        pshufb xmm4, xmm7;
        sha1nexte xmm2, xmm4;
        movdqa xmm1, xmm0;
        sha1rnds4 xmm0, xmm2, 0;
        sha1msg1 xmm3, xmm4;

        # round 8..11
        movdqa xmm5, [rsi + 32];
        pshufb xmm5, xmm7;
        sha1nexte xmm1, xmm5;
        movdqa xmm2, xmm0;
        sha1rnds4 xmm0, xmm1, 0;
        sha1msg1 xmm4, xmm5;
        pxor xmm3, xmm5;

        # round 12..15
        movdqa xmm6, [rsi + 48];
        pshufb xmm6, xmm7;
        sha1nexte xmm2, xmm6;
        movdqa xmm1, xmm0;
        sha1msg2 xmm3, xmm6;
        sha1rnds4 xmm0, xmm2, 0;
        sha1msg1 xmm5, xmm6;
        pxor xmm4, xmm6;

        # round 16..19
        sha1nexte xmm1, xmm3;
        movdqa xmm2, xmm0;
        sha1msg2 xmm4, xmm3;
        sha1rnds4 xmm0, xmm1, 0;
        sha1msg1 xmm6, xmm3;
        pxor xmm5, xmm3;

        # round 20..23
        sha1nexte xmm2, xmm4;
        movdqa xmm1, xmm0;
        sha1msg2 xmm5, xmm4;
        sha1rnds4 xmm0, xmm2, 0;
        sha1msg1 xmm3, xmm4;
        pxor xmm6, xmm4;

        # round 24..27
        sha1nexte xmm1, xmm5;
        movdqa xmm2, xmm0;
        sha1msg2 xmm6, xmm5;
        sha1rnds4 xmm0, xmm1, 0;
        sha1msg1 xmm5, xmm4;
        pxor xmm3, xmm5;



        vmovdqa ymm0, [rdx];

        vmovdqa ymm1, [rsi + 0];
        vmovdqa ymm2, [rsi + 32];

        vpshufb ymm1, ymm1, ymm0;
        vpshufb ymm2, ymm2, ymm0;

        vmovdqa [rdi + 0], ymm1;
        vmovdqa [rdi + 32], ymm2;

        vmovdqa xmm1, [rdx + 32];
        vmovdqa xmm2, [rdx + 48];

        add rdi, 64;
        mov ecx, 5;

    load_extended_w:
        vmovdqu xmm0, [rdi + 0 - 12];
        vpxor xmm0, xmm0, [rdi + 0 - 32];
        vpxor xmm0, xmm0, [rdi + 0 - 56];
        vpxor xmm0, xmm0, [rdi + 0 - 64];

        vpsllvd xmm3, xmm0, xmm1;
        vpsrlvd xmm4, xmm0, xmm2;

        vpor xmm0, xmm3, xmm4;
        vmovdqu [rdi + 0], xmm0;

        vmovdqu xmm0, [rdi + 12 - 12];
        vpxor xmm0, xmm0, [rdi + 12 - 32];
        vpxor xmm0, xmm0, [rdi + 12 - 56];
        vpxor xmm0, xmm0, [rdi + 12 - 64];

        vpsllvd xmm3, xmm0, xmm1;
        vpsrlvd xmm4, xmm0, xmm2;

        vpor xmm0, xmm3, xmm4;
        vmovdqu [rdi + 12], xmm0;

        vmovdqu xmm0, [rdi + 24 - 12];
        vpxor xmm0, xmm0, [rdi + 24 - 32];
        vpxor xmm0, xmm0, [rdi + 24 - 56];
        vpxor xmm0, xmm0, [rdi + 24 - 64];

        vpsllvd xmm3, xmm0, xmm1;
        vpsrlvd xmm4, xmm0, xmm2;

        vpor xmm0, xmm3, xmm4;
        vmovdqu [rdi + 24], xmm0;

        vmovdqu xmm0, [rdi + 36 - 12];
        vpxor xmm0, xmm0, [rdi + 36 - 32];
        vpxor xmm0, xmm0, [rdi + 36 - 56];
        vpxor xmm0, xmm0, [rdi + 36 - 64];

        vpsllvd xmm3, xmm0, xmm1;
        vpsrlvd xmm4, xmm0, xmm2;

        vpor xmm0, xmm3, xmm4;
        vmovdqu [rdi + 36], xmm0;

        add rdi, 48;
        dec ecx;
        jnz load_extended_w;

        vmovdqu xmm0, [rdi + 0 - 12];
        vpxor xmm0, xmm0, [rdi + 0 - 32];
        vpxor xmm0, xmm0, [rdi + 0 - 56];
        vpxor xmm0, xmm0, [rdi + 0 - 64];

        vpsllvd xmm3, xmm0, xmm1;
        vpsrlvd xmm4, xmm0, xmm2;

        vpor xmm0, xmm3, xmm4;
        vmovdqu [rdi + 0], xmm0;

        mov r9d, [rdi + 12 - 12];
        xor r9d, [rdi + 12 - 32];
        xor r9d, [rdi + 12 - 56];
        xor r9d, [rdi + 12 - 64];

        rol r9d, 1;
        mov [rdi + 12], r9d;

        ret
"
);

extern "C" {
    fn _sha1_update(dst: *mut u32, src: *const u8, mask: *const u8);
}

#[repr(C, align(16))]
pub struct Sha1 {
    w: [u32; 80],
    s: [u8; 64],
    h: [u32; 5],
}

impl Sha1 {
    pub fn new() -> Sha1 {
        Sha1 {
            h: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
            w: [0; 80],
            s: [
                3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13, 12, 3, 2, 1, 0, 7, 6, 5, 4, 11, 10, 9, 8, 15, 14, 13,
                12, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 31, 0, 0, 0, 31, 0, 0, 0, 31, 0, 0, 0, 31, 0, 0, 0,
            ],
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
                _sha1_update(&mut self.w as *mut u32, ptr.add(i * 64), &self.s as *const u8);

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
