use ::core::arch;

arch::global_asm!(
    "
    .global _sha1_update;

    _sha1_update:
        movdqa xmm0, [rdi]            # [abcd] = 10325476 98badcfe efcdab89 67452301
        movdqa xmm1, [rdi+16]         # [e0]   = 00000000 00000000 00000000 c3d2e1f0
        movdqa xmm7, [rdx]            # [shuf] = 0c0d0e0f 08090a0b 04050607 00010203

        # round 0..3
        movdqa xmm3, [rsi]            # [msg0] = 00000080 00000000 00000000 00000000
        pshufb xmm3, xmm7             # [msg0] = 00000000 00000000 00000000 80000000

        paddd xmm1, xmm3              # [e0]   = 00000000 00000000 00000000 43d2e1f0
        movdqa xmm2, xmm0             # [e1]   = 10325476 98badcfe efcdab89 67452301
        sha1rnds4 xmm0, xmm1, 0       # [abcd] = c7ed262c 1750f8dc 158d2f62 cdecfb5d

        # round 4..7
        movdqa xmm4, [rsi+16]         # [msg1] = 00000000 00000000 00000000 00000000
        pshufb xmm4, xmm7             # [msg1] = 00000000 00000000 00000000 00000000
        sha1nexte xmm2, xmm4          # [e1]   = 00000000 00000000 00000000 59d148c0
        movdqa xmm1, xmm0             # [e0]   = c7ed262c 1750f8dc 158d2f62 cdecfb5d
        sha1rnds4 xmm0, xmm2, 0       # [abcd] = 9254d597 b912add9 c09d7f27 87074800
        sha1msg1 xmm3, xmm4           # [msg0] = 00000000 00000000 00000000 80000000

        # round 8..11
        movdqa xmm5, [rsi+32]         # [msg2] = 00000000 00000000 00000000 00000000
        pshufb xmm5, xmm7             # [msg2] = 00000000 00000000 00000000 00000000
        sha1nexte xmm1, xmm5          # [e0]   = 00000000 00000000 00000000 737b3ed7
        movdqa xmm2, xmm0             # [e1]   = 9254d597 b912add9 c09d7f27 87074800
        sha1rnds4 xmm0, xmm1, 0       # [abcd] = 504dd984 72f6ffcc 40166973 adc0e0ca
        sha1msg1 xmm4, xmm5           # [msg1] = 00000000 00000000 00000000 00000000
        pxor xmm3, xmm5               # [msg0] = 00000000 00000000 00000000 80000000

        # round 12..15
        movdqa xmm6, [rsi+48]         # [msg2] = 00000000 00000000 00000000 00000000
        pshufb xmm6, xmm7             # [msg2] = 00000000 00000000 00000000 00000000
        sha1nexte xmm2, xmm6          # [e1]   = 00000000 00000000 00000000 21c1d200
        movdqa xmm1, xmm0             # [e0]   = 504dd984 72f6ffcc 40166973 adc0e0ca
        sha1msg2 xmm3, xmm6           # [msg0] = 00000002 00000000 00000000 00000001
        sha1rnds4 xmm0, xmm2, 0       # [abcd] = a13017ac 4544b22e 40182905 d8fd6547
        sha1msg1 xmm5, xmm6           # [msg2] = 00000000 00000000 00000000 00000000
        pxor xmm4, xmm6               # [msg1] = 00000000 00000000 00000000 00000000

        # round 16..19
        sha1nexte xmm1, xmm3          # [e0]   = 00000002 00000000 00000000 ab703833 x
        movdqa xmm2, xmm0             # [e1]   = a13017ac 4544b22e 40182905 d8fd6547
        sha1msg2 xmm4, xmm3           # [msg1] = 00000000 00000004 00000000 00000000
        sha1rnds4 xmm0, xmm1, 0       # [abcd] = c1afe45c 8a2a5483 0b3088dd e758e8da
        sha1msg1 xmm6, xmm3           # [msg3] = 00000000 00000001 00000000 00000000
        pxor xmm5, xmm3               # [msg2] = 00000002 00000000 00000000 00000001

        # round 20..23
        sha1nexte xmm2, xmm4          # [e1]   = 00000000 00000004 00000000 f63f5951
        movdqa xmm1, xmm0             # [e0]   = c1afe45c 8a2a5483 0b3088dd e758e8da
        sha1msg2 xmm5, xmm4           # [msg2] = 00000000 00000000 00000008 00000002
        sha1rnds4 xmm0, xmm2, 1       # [abcd] = 243ae614 5f6ede1f 1c64d028 1e97b73a
        sha1msg1 xmm3, xmm4           # [msg0] = 00000002 00000000 00000002 00000001
        pxor xmm6, xmm4               # [msg3] = 00000000 00000005 00000000 00000000

        # round 24..27
        sha1nexte xmm1, xmm5          # [e0]   = 00000000 00000000 00000008 b9d63a38 x
        movdqa xmm2, xmm0             # [e1]   = 243ae614 5f6ede1f 1c64d028 1e97b73a
        sha1msg2 xmm6, xmm5           # [msg3] = 00000020 0000000a 00000000 00000010
        sha1rnds4 xmm0, xmm1, 1       # [abcd] = d8b5fd4f 0d3cf5b6 4f2ed1c1 c7b11e2d
        sha1msg1 xmm4, xmm5           # [msg1] = 00000008 00000006 00000000 00000004
        pxor xmm3, xmm5               # [msg0] = 00000002 00000000 0000000a 00000003

        # round 28..31
        sha1nexte xmm2, xmm6          # [e1]   = 00000020 0000000a 00000000 87a5edde x
        movdqa xmm1, xmm0             # [e0]   = d8b5fd4f 0d3cf5b6 4f2ed1c1 c7b11e2d
        sha1msg2 xmm3, xmm6           # [msg0] = 00000008 00000040 00000000 00000006
        sha1rnds4 xmm0, xmm2, 1       # [abcd] = e1d2de1b f29155b2 6a2e466e 62ea3d59
        sha1msg1 xmm5, xmm6           # [msg2] = 00000000 00000010 00000008 00000002
        pxor xmm4, xmm6               # [msg1] = 00000028 0000000c 00000000 00000014

        # round 32..35
        sha1nexte xmm1, xmm3          # [e0]   = 00000008 00000040 00000000 71ec4791 x
        movdqa xmm2, xmm0             # [e1]   = e1d2de1b f29155b2 6a2e466e 62ea3d59
        sha1msg2 xmm4, xmm3           # [msg1] = 00000000 00000008 00000080 00000028
        sha1rnds4 xmm0, xmm1, 1       # [abcd] = 6ddeeb09 92c4d1f8 391ef0c4 abbab988
        sha1msg1 xmm6, xmm3           # [msg3] = 00000020 0000000c 00000020 0000001a
        pxor xmm5, xmm3               # [msg2] = 00000008 00000050 00000008 00000004

        # round 36..39
        sha1nexte xmm2, xmm4          # [e1]   = 00000000 00000008 00000080 58ba8f7e x
        movdqa xmm1, xmm0             # [e0]   = 6ddeeb09 92c4d1f8 391ef0c4 abbab988
        sha1msg2 xmm5, xmm4           # [msg2] = 00000200 000000a0 00000000 00000108
        sha1rnds4 xmm0, xmm2, 1       # [abcd] = 413c1d9a ec805e22 62273351 9bdbdd71
        sha1msg1 xmm3, xmm4           # [msg0] = 00000088 00000068 00000008 00000046
        pxor xmm6, xmm4               # [msg3] = 00000020 00000004 000000a0 00000032

        # round 40..43
        sha1nexte xmm1, xmm5          # [e0]   = 00000200 000000a0 00000000 2aeeaf6a x
        movdqa xmm2, xmm0             # [e1]   = 413c1d9a ec805e22 62273351 9bdbdd71
        sha1msg2 xmm6, xmm5           # [msg3] = 00000088 00000408 00000000 00000064
        sha1rnds4 xmm0, xmm1, 2       # [abcd] = e56a8e62 178a3a16 95642485 fa950aba
        sha1msg1 xmm4, xmm5           # [msg1] = 00000000 00000100 00000080 00000020
        pxor xmm3, xmm5               # [msg0] = 00000288 000000c8 00000008 0000014e

        # round 44..47
        sha1nexte xmm2, xmm6          # [e1]   = 00000088 00000408 00000000 66f6f7c0 x
        movdqa xmm1, xmm0             # [e0]   = e56a8e62 178a3a16 95642485 fa950aba
        sha1msg2 xmm3, xmm6           # [msg0] = 00000028 00000080 00000800 0000029c
        sha1rnds4 xmm0, xmm2, 2       # [abcd] = 77878e80 ebf9a56a a195ba90 e6d39f43
        sha1msg1 xmm5, xmm6           # [msg2] = 00000200 000000c4 00000200 000001a8
        pxor xmm4, xmm6               # [msg1] = 00000088 00000508 00000080 00000044

        # round 48..51
        sha1nexte xmm1, xmm3          # [e0]   = 00000028 00000080 00000800 bea5454a x
        movdqa xmm2, xmm0             # [e1]   = 77878e80 ebf9a56a a195ba90 e6d39f43
        sha1msg2 xmm4, xmm3           # [msg1] = 00002000 00000a40 00000000 00001088
        sha1rnds4 xmm0, xmm1, 2       # [abcd] = 82f2a648 daba09bf 01ff3253 e2581ce0
        sha1msg1 xmm6, xmm3           # [msg3] = 00000888 00000694 00000088 0000046c
        pxor xmm5, xmm3               # [msg2] = 00000228 00000044 00000a00 00000334

        # round 52..55
        sha1nexte xmm2, xmm4          # [e1]   = 00002000 00000a40 00000000 f9b4f858 x
        movdqa xmm1, xmm0             # [e0]   = 82f2a648 daba09bf 01ff3253 e2581ce0
        sha1msg2 xmm5, xmm4           # [msg2] = 00000880 00004088 00000080 00000668
        sha1rnds4 xmm0, xmm2, 2       # [abcd] = d5b39cea ab95b950 8590c0e8 be4a4bea
        sha1msg1 xmm3, xmm4           # [msg0] = 00000028 00001008 00000828 0000021c
        pxor xmm6, xmm4               # [msg3] = 00002888 00000cd4 00000088 000014e4

        # round 56..59
        sha1nexte xmm1, xmm5          # [e0]   = 00000880 00004088 00000080 38960da0 x
        movdqa xmm2, xmm0             # [e1]   = d5b39cea ab95b950 8590c0e8 be4a4bea
        sha1msg2 xmm6, xmm5           # [msg3] = 00000080 000008a8 00008000 000028c8
        sha1rnds4 xmm0, xmm1, 2       # [abcd] = c5a3382e b86beac8 982bcbca 9b9d2913
        sha1msg1 xmm4, xmm5           # [msg1] = 00002080 00000c28 00002000 00001ac8
        pxor xmm3, xmm5               # [msg0] = 000008a8 00005080 000008a8 00000474

        # round 60..63
        sha1nexte xmm2, xmm6          # [e1]   = 00000080 000008a8 00008000 af92bbc2 x
        movdqa xmm1, xmm0             # [e0]   = c5a3382e b86beac8 982bcbca 9b9d2913
        sha1msg2 xmm3, xmm6           # [msg0] = 00020080 0000a000 00000000 000108e8
        sha1rnds4 xmm0, xmm2, 3       # [abcd] = f4df6e4d e16e7489 cd98fbb7 bb0f226f
        sha1msg1 xmm5, xmm6           # [msg2] = 00008880 00006840 00000800 000046e0
        pxor xmm4, xmm6               # [msg1] = 00002000 00000480 0000a000 00003200

        # round 64..67
        sha1nexte xmm1, xmm3          # [e0]   = 00020080 0000a000 00000000 e6e8532c x
        movdqa xmm2, xmm0             # [e1]   = f4df6e4d e16e7489 cd98fbb7 bb0f226f
        sha1msg2 xmm4, xmm3           # [msg1] = 00008800 00040800 00000000 00006400
        sha1rnds4 xmm0, xmm1, 3       # [abcd] = 3ad6511b f4dc8972 111341f3 e79afbf0
        sha1msg1 xmm6, xmm3           # [msg3] = 00000080 00010040 00008080 00002060
        pxor xmm5, xmm3               # [msg2] = 00028800 0000c840 00000800 00014e08

        # round 68..71
        sha1nexte xmm2, xmm4          # [e1]   = 00008800 00040800 00000000 eec42c9b x
        movdqa xmm1, xmm0             # [e0]   = 3ad6511b f4dc8972 111341f3 e79afbf0
        sha1msg2 xmm5, xmm4           # [msg2] = 00002820 00008080 00080000 00029c10
        sha1rnds4 xmm0, xmm2, 3       # [abcd] = e2e80189 140f1eb8 3cd517f9 b47ddf0e
        pxor xmm6, xmm4               # [msg3] = 00008880 00050840 00008080 00004460

        # round 72..75
        sha1nexte xmm1, xmm5          # [e0]   = 00002820 00008080 00080000 39e95b0c x
        movdqa xmm2, xmm0             # [e1]   = e2e80189 140f1eb8 3cd517f9 b47ddf0e
        sha1msg2 xmm6, xmm5           # [msg3] = 00200080 000a40c0 00000000 001088c0
        sha1rnds4 xmm0, xmm1, 3       # [abcd] = 178e81e0 98f6cdec 15e98d17 b0149467

        # round 76..79
        sha1nexte xmm2, xmm6          # [e1]   = 00200080 000a40c0 00000000 ad300083 x
        movdqa xmm1, xmm0             # [e0]   = 178e81e0 98f6cdec 15e98d17 b0149467
        sha1rnds4 xmm0, xmm2, 3       # [abcd] = 852dc41a 999ae2f1 6e9d9f84 72f480ed

        sha1nexte xmm1, [rdi+16]      # [e0]   = 00000000 00000000 00000000 afd80709
        paddd xmm0, [rdi]             # [abcd] = 95601890 3255bfef 5e6b4b0d da39a3ee

        movdqa [rdi], xmm0            # [abcd] = 10325476 98badcfe efcdab89 67452301
        movdqa [rdi+16], xmm1         # [e0]   = 00000000 00000000 00000000 c3d2e1f0

        ret
"
);

extern "C" {
    fn _sha1_update(dst: *mut u32, src: *const u8, mask: *const u8);
}

#[repr(C, align(16))]
pub struct Sha1 {
    h: [u32; 12],
}

impl Sha1 {
    pub fn new() -> Sha1 {
        Sha1 {
            h: [0x10325476, 0x98badcfe, 0xefcdab89, 0x67452301, 0, 0, 0, 0xc3d2e1f0, 0x0c0d0e0f, 0x08090a0b, 0x04050607, 0x00010203],
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

        [this.h[3], this.h[2], this.h[1], this.h[0], this.h[7]]
    }

    pub fn update(mut self, ptr: *const u8, len: usize) -> Self {
        for i in 0..(len / 64) {
            unsafe {
                let src = ptr.add(i * 64);
                let dst = &mut self.h as *mut u32;
                let mask = dst.add(8) as *const u8;
                _sha1_update(dst, src, mask);
            }
        }

        self
    }
}
