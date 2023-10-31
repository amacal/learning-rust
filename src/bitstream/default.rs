use crate::bitstream::header::{BitStream, BitStreamError, BitStreamResult};

pub struct BitStreamDefault<const T: usize> {
    buffer: Box<[u8; T]>, // buffer to hold the slice of data
    boundary: usize,      // boundary within the buffer
    processed: usize,     // offset next byte to be processed
    mask: u8,             // mask next bit to be processed
}

impl<const T: usize> BitStreamDefault<T> {
    pub fn new() -> Self {
        Self {
            buffer: Box::new([0; T]),
            boundary: 0,
            processed: 0,
            mask: 0x01,
        }
    }

    #[inline]
    fn bufferable(&self) -> usize {
        T - (self.boundary - self.processed)
    }
}

impl<const T: usize> BitStream for BitStreamDefault<T> {
    fn available(&self) -> usize {
        8 * (self.boundary - self.processed - 1)
            + match self.mask {
                0x01 => 8,
                0x02 => 7,
                0x04 => 6,
                0x08 => 5,
                0x10 => 4,
                0x20 => 3,
                0x40 => 2,
                0x80 => 1,
                _ => panic!("Something is wrong with the bitmask!"),
            }
    }

    fn appendable(&self) -> Option<usize> {
        // only appendable if at least half buffer can be appended
        match self.bufferable() {
            value if value < T / 2 => None,
            value => Some(value),
        }
    }

    fn append(&mut self, data: &[u8]) -> BitStreamResult<()> {
        if data.len() > self.bufferable() {
            return BitStreamError::raise_too_much_data(data.len(), self.bufferable());
        }

        // move already processed data to the beginning of the buffer
        self.buffer.copy_within(self.processed..self.boundary, 0);

        self.boundary -= self.processed;
        self.processed = 0;

        // copy new data just after boundary
        self.buffer[self.boundary..self.boundary + data.len()].copy_from_slice(data);
        self.boundary += data.len();

        Ok(())
    }

    fn next_bit(&mut self) -> Option<u8> {
        let bit = match &self.buffer.get(0..self.boundary) {
            None => return None,
            Some(slice) => match slice.get(self.processed) {
                None => return None,
                Some(&value) => ((value & self.mask) > 0) as u8,
            },
        };

        if self.mask == 0x80 {
            self.mask = 1;
            self.processed += 1;
        } else {
            self.mask <<= 1;
        }

        Some(bit)
    }

    fn next_bit_unchecked(&mut self) -> u8 {
        unsafe {
            let &byte = self.buffer.get_unchecked(self.processed);
            let bit = ((byte & self.mask) > 0) as u8;

            if self.mask == 0x80 {
                self.mask = 1;
                self.processed += 1;
            } else {
                self.mask <<= 1;
            }

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
        self.mask = 0x01;

        let data = match &self.buffer.get(0..self.boundary) {
            None => None,
            Some(slice) => match slice.get(self.processed..self.processed + count) {
                None => None,
                Some(value) => Some(value),
            },
        };

        let data = match &data {
            Some(data) => data,
            None => return BitStreamError::raise_not_enough_data(count, self.boundary - self.processed),
        };

        let mut target = vec![0; data.len()];
        target[..].copy_from_slice(data);

        self.processed += data.len();
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bitstream<const T: usize>(data: &[u8]) -> BitStreamDefault<T> {
        let mut bitstream = BitStreamDefault::new();
        bitstream.append(data).unwrap();
        bitstream
    }

    #[test]
    fn creates_bitstream() {
        let bitstream: BitStreamDefault<32> = BitStreamDefault::new();

        assert_eq!(bitstream.processed, 0);
        assert_eq!(bitstream.boundary, 0);
        assert_eq!(bitstream.buffer.len(), 32);
    }

    #[test]
    fn appends_too_big_slice() {
        let mut bitstream: BitStreamDefault<2> = BitStreamDefault::new();
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
    fn reads_bit_by_bit() {
        let data = [0x27, 0x00];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(1));

        assert_eq!(bitstream.processed, 1);
    }

    #[test]
    fn reads_bits_in_pairs() {
        let data = [0x27, 0x00];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(2), Some(3));
        assert_eq!(bitstream.next_bits(2), Some(1));
        assert_eq!(bitstream.next_bits(2), Some(2));
        assert_eq!(bitstream.next_bits(2), Some(0));

        assert_eq!(bitstream.processed, 1);
    }

    #[test]
    fn aligns_to_next_byte_3bit() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(2), Some(3));
        assert_eq!(bitstream.next_bytes(1), Ok(vec![0x31]));

        assert_eq!(bitstream.processed, 2);
    }

    #[test]
    fn aligns_to_next_byte_0bit() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.next_bytes(1), Ok(vec![0x27]));
        assert_eq!(bitstream.processed, 1);
    }

    #[test]
    fn reads_bytes() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.next_bytes(2), Ok(vec![0x27, 0x31]));
        assert_eq!(bitstream.processed, 2);
    }

    #[test]
    fn reads_bytes_too_many() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

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

        assert_eq!(bitstream.processed, 0);
    }

    #[test]
    fn reads_till_last_bit() {
        let data = [0x27];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(8), Some(0x27));
        assert_eq!(bitstream.processed, 1);
    }

    #[test]
    fn reads_till_last_bit_plus_one() {
        let data = [0x27];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(8), Some(0x27));
        assert_eq!(bitstream.next_bit(), None);
        assert_eq!(bitstream.processed, 1);
    }

    #[test]
    fn detects_appendable_when_half_consumed() {
        let data = [0; 32];
        let mut bitstream: BitStreamDefault<32> = bitstream(&data);

        assert_eq!(bitstream.appendable(), None);
        assert_eq!(bitstream.next_bytes(15), Ok(vec![0; 15]));

        assert_eq!(bitstream.appendable(), None);
        assert_eq!(bitstream.next_bytes(1), Ok(vec![0]));

        assert_eq!(bitstream.appendable(), Some(16));
    }
}
