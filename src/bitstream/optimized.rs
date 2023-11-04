use std::arch::x86_64::*;

use crate::bitstream::header::{BitStream, BitStreamError, BitStreamResult};

pub struct BitStreamOptimized<const TBYTES: usize, const TBITS: usize> {
    bytes: Box<[u8; TBYTES]>,
    bits: Box<[u8; TBITS]>,
    bits_boundary: usize,
    bits_processed: usize,
}

impl<const TBYTES: usize, const TBITS: usize> BitStreamOptimized<TBYTES, TBITS> {
    pub fn new() -> Self {
        Self {
            bytes: Box::new([0; TBYTES]),
            bits: Box::new([0; TBITS]),
            bits_boundary: 0,
            bits_processed: 0,
        }
    }

    #[inline]
    fn bufferable(&self) -> usize {
        (TBITS - (self.bits_boundary - self.bits_processed)) >> 3
    }
}

impl<const TBYTES: usize, const TBITS: usize> BitStream for BitStreamOptimized<TBYTES, TBITS> {
    fn available(&self) -> usize {
        self.bits_boundary - self.bits_processed
    }

    fn appendable(&self) -> Option<usize> {
        // only appendable if at least half buffer can be appended
        match self.bufferable() {
            value if value < TBYTES / 2 => None,
            value => Some(value),
        }
    }

    fn append(&mut self, data: &[u8]) -> BitStreamResult<()> {
        if data.len() > self.bufferable() {
            return BitStreamError::raise_too_much_data(data.len(), self.bufferable());
        }

        // move already processed data to the beginning of the buffer
        let bytes_processed = self.bits_processed >> 3;
        let bits_processed = bytes_processed << 3;

        let bits_offset = self.bits_processed % 8;
        let bytes_boundary = self.bits_boundary >> 3;

        self.bits.copy_within(bits_processed..self.bits_boundary, 0);
        self.bytes.copy_within(bytes_processed..bytes_boundary, 0);

        self.bits_boundary -= bits_processed;
        self.bits_processed = bits_offset;

        // copy new data just after boundary and recompute bits
        let bytes_boundary = self.bits_boundary >> 3;
        self.bytes[bytes_boundary..bytes_boundary + data.len()].copy_from_slice(data);
        self.bits_boundary += data.len() << 3;

        unsafe {
            let and_mask256 = _mm256_set_epi32(
                0b1000_0000_1000_0000_1000_0000_1000_0000u32 as i32,
                0b0100_0000_0100_0000_0100_0000_0100_0000u32 as i32,
                0b0010_0000_0010_0000_0010_0000_0010_0000u32 as i32,
                0b0001_0000_0001_0000_0001_0000_0001_0000u32 as i32,
                0b0000_1000_0000_1000_0000_1000_0000_1000u32 as i32,
                0b0000_0100_0000_0100_0000_0100_0000_0100u32 as i32,
                0b0000_0010_0000_0010_0000_0010_0000_0010u32 as i32,
                0b0000_0001_0000_0001_0000_0001_0000_0001u32 as i32,
            );

            let shuffle_mask256 = _mm256_setr_epi8(
                0, 4, 8, 12, 1, 5, 9, 13,
                2, 6, 10, 14, 3, 7, 11, 15,
                0, 4, 8, 12, 1, 5, 9, 13,
                2, 6, 10, 14, 3, 7, 11, 15,
            );

            let permute_mask256 = _mm256_setr_epi32(
                0, 4, 1, 5, 2, 6, 3, 7
            );

            let mut src = self.bytes[bytes_boundary..].as_ptr() as *const i32;
            let mut dst = self.bits[bytes_boundary << 3..].as_mut_ptr() as *mut __m256i;

            for _ in 0..(data.len() / 4) {
                let int: i32 = std::ptr::read_unaligned(src);
                let reg256 = _mm256_set1_epi32(int);

                let and_result = _mm256_and_si256(reg256, and_mask256);
                let shuffle_result = _mm256_shuffle_epi8(and_result, shuffle_mask256);
                let permute_result = _mm256_permutevar8x32_epi32(shuffle_result, permute_mask256);

                let cmp_mask = _mm256_cmpeq_epi8(permute_result, _mm256_set1_epi8(0));
                let add_result = _mm256_add_epi8(cmp_mask, _mm256_set1_epi8(1));

                _mm256_storeu_si256(dst, add_result);

                src = src.add(1);
                dst = dst.add(1);

                //println!("avx {:?}", &self.bits[bytes_boundary << 3..(bytes_boundary<<3) + 16]);
            }
        }

        unsafe {
            for index in bytes_boundary + (data.len() / 4)..bytes_boundary + data.len() {
                for offset in 0..8 {
                    let bit = self.bits.get_unchecked_mut((index << 3) + offset);
                    let value = if self.bytes[index] & (1 << offset) != 0 { 1 } else { 0 };

                    *bit = value;
                }
            }

            //println!("tst {:?}", &self.bits[bytes_boundary << 3..(bytes_boundary<<3) + 16]);
        }

        Ok(())
    }

    fn next_bit(&mut self) -> Option<u8> {
        match self.bits.get(0..self.bits_boundary) {
            None => return None,
            Some(bits) => match bits.get(self.bits_processed) {
                None => return None,
                Some(&bit) => {
                    self.bits_processed += 1;
                    Some(bit)
                }
            }
        }
    }

    fn next_bit_unchecked(&mut self) -> u8 {
        unsafe {
            let &bit = self.bits.get_unchecked(self.bits_processed);
            self.bits_processed += 1;
            bit
        }
    }

    fn next_bits(&mut self, count: usize) -> Option<u16> {
        let mut outcome: u16 = 0;

        for i in 0..count {
            outcome = match self.next_bit() {
                Some(bit) => outcome | ((bit as u16) << i),
                None => return None,
            };
        }

        Some(outcome)
    }

    fn next_bits_unchecked(&mut self, count: usize) -> u16 {
        let mut outcome: u16 = 0;

        for i in 0..count {
            outcome |= (self.next_bit_unchecked() as u16) << i;
        }

        outcome
    }

    fn next_bytes(&mut self, count: usize) -> BitStreamResult<Vec<u8>> {
        if self.bits_processed % 8 != 0 {
            self.bits_processed = ((self.bits_processed + 8) >> 3) << 3;
        }

        let bytes_boundary = self.bits_boundary >> 3;
        let bytes_processed = self.bits_processed >> 3;

        let data = match &self.bytes.get(0..bytes_boundary) {
            None => None,
            Some(slice) => match slice.get(bytes_processed..bytes_processed + count) {
                None => None,
                Some(value) => Some(value),
            },
        };

        let data = match &data {
            Some(data) => data,
            None => return BitStreamError::raise_not_enough_data(count, bytes_boundary - bytes_processed),
        };

        let mut target = vec![0; data.len()];
        target[..].copy_from_slice(data);

        self.bits_processed += data.len() << 3;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bitstream<const TBYTES: usize, const TBITS: usize>(data: &[u8]) -> BitStreamOptimized<TBYTES, TBITS> {
        let mut bitstream = BitStreamOptimized::new();
        bitstream.append(data).unwrap();
        bitstream
    }

    #[test]
    fn creates_bitstream() {
        let bitstream: BitStreamOptimized<32, 256> = BitStreamOptimized::new();

        assert_eq!(bitstream.bytes.len(), 32);
        assert_eq!(bitstream.bits.len(), 256);

        assert_eq!(bitstream.bits_processed, 0);
        assert_eq!(bitstream.bits_boundary, 0);
    }

    #[test]
    fn appends_too_big_slice() {
        let mut bitstream: BitStreamOptimized<2, 16> = BitStreamOptimized::new();
        let data = [0, 1, 2, 3];

        match bitstream.append(data.as_ref()) {
            Ok(_) => assert!(false),
            Err(error) => match error {
                BitStreamError::TooMuchData { passed, acceptable } => {
                    assert_eq!(passed, 4);
                    assert_eq!(acceptable, 2);
                }
                _ => assert!(false),
            },
        }
    }

    #[test]
    fn appends_twice() {
        let mut bitstream: BitStreamOptimized<32, 256> = BitStreamOptimized::new();
        let data = [0x27];

        assert_eq!(bitstream.append(data.as_ref()), Ok(()));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(0));

        assert_eq!(bitstream.bits_processed, 5);
        assert_eq!(bitstream.bits_boundary, 8);
        assert_eq!(bitstream.append(data.as_ref()), Ok(()));
        assert_eq!(bitstream.bits_processed, 5);
        assert_eq!(bitstream.bits_boundary, 16);

        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bits(2), Some(3));
        assert_eq!(bitstream.next_bits(2), Some(1));
        assert_eq!(bitstream.next_bits(2), Some(2));
        assert_eq!(bitstream.next_bits(2), Some(0));
    }

    #[test]
    fn reads_bit_by_bit() {
        let data = [0x27, 0x00];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(1));

        assert_eq!(bitstream.bits_processed, 6);
    }

    #[test]
    fn reads_bits_in_pairs() {
        let data = [0x27, 0x00];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.next_bits(2), Some(3));
        assert_eq!(bitstream.next_bits(2), Some(1));
        assert_eq!(bitstream.next_bits(2), Some(2));
        assert_eq!(bitstream.next_bits(2), Some(0));

        assert_eq!(bitstream.bits_processed, 8);
    }

    #[test]
    fn aligns_to_next_byte_3bit() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.next_bits(2), Some(3));
        assert_eq!(bitstream.next_bytes(1), Ok(vec![0x31]));

        assert_eq!(bitstream.bits_processed, 16);
    }

    #[test]
    fn aligns_to_next_byte_0bit() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.next_bytes(1), Ok(vec![0x27]));
        assert_eq!(bitstream.bits_processed, 8);
    }

    #[test]
    fn reads_bytes() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.next_bytes(2), Ok(vec![0x27, 0x31]));
        assert_eq!(bitstream.bits_processed, 16);
    }

    #[test]
    fn reads_bytes_too_many() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        match bitstream.next_bytes(3) {
            Ok(_) => assert!(false),
            Err(error) => match error {
                BitStreamError::NotEnoughData { requested, available } => {
                    assert_eq!(requested, 3);
                    assert_eq!(available, 2);
                }
                _ => assert!(false),
            },
        };

        assert_eq!(bitstream.bits_processed, 0);
    }

    #[test]
    fn reads_till_last_bit() {
        let data = [0x27];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.next_bits(8), Some(0x27));
        assert_eq!(bitstream.bits_processed, 8);
    }

    #[test]
    fn reads_till_last_bit_plus_one() {
        let data = [0x27];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.next_bits(8), Some(0x27));
        assert_eq!(bitstream.next_bit(), None);
        assert_eq!(bitstream.bits_processed, 8);
    }

    #[test]
    fn detects_appendable_when_half_consumed() {
        let data = [0; 32];
        let mut bitstream: BitStreamOptimized<32, 256> = bitstream(&data);

        assert_eq!(bitstream.appendable(), None);
        assert_eq!(bitstream.next_bytes(15), Ok(vec![0; 15]));

        assert_eq!(bitstream.appendable(), None);
        assert_eq!(bitstream.next_bytes(1), Ok(vec![0]));

        assert_eq!(bitstream.appendable(), Some(16));
    }
}
