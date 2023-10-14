pub struct BitStreamS<const T: usize> {
    buffer: Box<[u8; T]>, // buffer to hold the slice of data
    boundary: usize,      // boundary within the buffer
    current: Option<u8>,  // byte currently being processed
    processed: usize,     // offset next byte to be processed
    collected: usize,     // offset next byte to be collected
    mask: u8,             // mask next bit to be processed
    total: u64,           // total number of bytes processed
}

pub trait BitStream {
    fn appendable(&self) -> Option<usize>;
    fn collect(&mut self, data: Option<&mut [u8]>) -> usize;
    fn append(&mut self, data: &[u8]) -> BitStreamResult<()>;

    fn next_bit(&mut self) -> Option<u8>;
    fn next_bits(&mut self, count: usize) -> Option<u16>;
    fn next_bytes(&mut self, count: usize) -> BitStreamResult<Vec<u8>>;
}

pub type BitStreamResult<T> = Result<T, BitStreamError>;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum BitStreamError {
    #[error("Passed {passed} bytes, but stream can only accept {acceptable} bytes.")]
    TooMuchData { passed: usize, acceptable: usize },

    #[error("Requested {requested} number of bytes is more than available {available} bytes.")]
    NotEnoughData { requested: usize, available: usize },

    #[error("Appending new data requires collecting already processed data first ({collectable} bytes).")]
    NotCollectedData { collectable: usize },
}

impl BitStreamError {
    fn raise_too_much_data<T>(passed: usize, acceptable: usize) -> BitStreamResult<T> {
        Err(BitStreamError::TooMuchData {
            passed: passed,
            acceptable: acceptable,
        })
    }

    fn raise_not_enough_data<T>(requested: usize, available: usize) -> BitStreamResult<T> {
        Err(BitStreamError::NotEnoughData { requested: requested, available: available, })
    }

    fn raise_not_collected_data<T>(collectable: usize) -> BitStreamResult<T> {
        Err(BitStreamError::NotCollectedData {
            collectable: collectable,
        })
    }
}

impl<const T: usize> BitStreamS<T> {
    pub fn new() -> Self {
        Self {
            buffer: Box::new([0; T]),
            boundary: 0,
            current: None,
            processed: 0,
            collected: 0,
            mask: 0x01,
            total: 0,
        }
    }

    #[inline]
    fn bufferable(&self) -> usize {
        T - (self.boundary - self.processed)
    }

    fn next(&self) -> Option<u8> {
        match &self.buffer.get(0..self.boundary) {
            None => None,
            Some(slice) => match slice.get(self.processed) {
                None => None,
                Some(&value) => Some(value),
            },
        }
    }
}

impl<const T: usize> BitStream for BitStreamS<T> {
    fn appendable(&self) -> Option<usize> {
        // only appendable if at least half buffer can be appended
        match self.bufferable() {
            value if value < T / 2 => None,
            value => Some(value),
        }
    }

    fn collect(&mut self, data: Option<&mut [u8]>) -> usize {
        // compute actual available bytes
        let available = match &data {
            Some(slice) => std::cmp::min(slice.len(), self.processed - self.collected),
            None => self.processed - self.collected,
        };

        // copy them into buffer if provided
        if available > 0 {
            if let Some(slice) = data {
                slice[0..available].copy_from_slice(&self.buffer[0..available]);
            }
        }

        // shift offsets
        self.collected += available;
        available
    }

    fn append(&mut self, data: &[u8]) -> BitStreamResult<()> {
        if data.len() > self.bufferable() {
            return BitStreamError::raise_too_much_data(data.len(), self.bufferable());
        }

        if self.collected != self.processed {
            return BitStreamError::raise_not_collected_data(self.processed - self.collected);
        }

        // move already processed data to the beginning of the buffer
        self.buffer.copy_within(self.processed..self.boundary, 0);
        self.boundary -= self.processed;

        self.processed = 0;
        self.collected = 0;

        // copy new data just after boundary
        self.buffer[self.boundary..self.boundary + data.len()].copy_from_slice(data);
        self.boundary += data.len();

        Ok(())
    }

    fn next_bit(&mut self) -> Option<u8> {
        // when there is no current byte we need to pick new one
        if let None = self.current {
            self.current = self.next();

            // if successful move all pointers
            if let Some(_) = self.current {
                self.processed += 1;
                self.total += 1;
            }
        }

        // bit will be either 0, 1 or no more data will be returned
        let bit = match self.current {
            None => return None,
            Some(value) => ((value & self.mask) > 0) as u8,
        };

        // shift left or remove current byte
        if self.mask == 0x80 {
            self.current = None;
            self.mask = 1;
        } else {
            self.mask <<= 1;
        }

        Some(bit)
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

    fn next_bytes(&mut self, count: usize) -> BitStreamResult<Vec<u8>> {
        self.mask = 0x01;
        self.current = None;

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
        self.total += data.len() as u64;

        self.current = None;
        Ok(target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bitstream<const T: usize>(data: &[u8]) -> BitStreamS<T> {
        let mut bitstream = BitStreamS::new();
        bitstream.append(data).unwrap();
        bitstream
    }

    #[test]
    fn creates_bitstream() {
        let bitstream: BitStreamS<32> = BitStreamS::new();

        assert_eq!(bitstream.processed, 0);
        assert_eq!(bitstream.total, 0);
        assert_eq!(bitstream.boundary, 0);
        assert_eq!(bitstream.buffer.len(), 32);
    }


    #[test]
    fn appends_too_big_slice() {
        let mut bitstream: BitStreamS<2> = BitStreamS::new();
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
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(1));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(0));
        assert_eq!(bitstream.next_bit(), Some(1));

        assert_eq!(bitstream.processed, 1);
        assert_eq!(bitstream.total, 1);
    }

    #[test]
    fn reads_bits_in_pairs() {
        let data = [0x27, 0x00];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(2), Some(3));
        assert_eq!(bitstream.next_bits(2), Some(1));
        assert_eq!(bitstream.next_bits(2), Some(2));
        assert_eq!(bitstream.next_bits(2), Some(0));

        assert_eq!(bitstream.processed, 1);
        assert_eq!(bitstream.total, 1);
    }

    #[test]
    fn aligns_to_next_byte_3bit() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(2), Some(3));
        assert_eq!(bitstream.next_bytes(1), Ok(vec![0x31]));

        assert_eq!(bitstream.processed, 2);
        assert_eq!(bitstream.total, 2);
    }

    #[test]
    fn aligns_to_next_byte_0bit() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bytes(1), Ok(vec![0x27]));

        assert_eq!(bitstream.processed, 1);
        assert_eq!(bitstream.total, 1);
    }

    #[test]
    fn reads_bytes() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bytes(2), Ok(vec![0x27, 0x31]));

        assert_eq!(bitstream.processed, 2);
        assert_eq!(bitstream.total, 2);
    }

    #[test]
    fn reads_bytes_too_many() {
        let data = [0x27, 0x31];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        match bitstream.next_bytes(3) {
            Ok(_) => assert!(false),
            Err(error) => match error {
                BitStreamError::NotEnoughData { requested, available } => {
                    assert_eq!(requested, 3);
                    assert_eq!(available, 2);
                },
                _ => assert!(false),
            }
        };

        assert_eq!(bitstream.processed, 0);
        assert_eq!(bitstream.total, 0);
    }

    #[test]
    fn reads_till_last_bit() {
        let data = [0x27];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(8), Some(0x27));

        assert_eq!(bitstream.processed, 1);
        assert_eq!(bitstream.total, 1);
    }

    #[test]
    fn reads_till_last_bit_plus_one() {
        let data = [0x27];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(8), Some(0x27));
        assert_eq!(bitstream.next_bit(), None);

        assert_eq!(bitstream.processed, 1);
        assert_eq!(bitstream.total, 1);
    }

    #[test]
    fn detects_appendable_when_half_consumed() {
        let data = [0; 32];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.appendable(), None);
        assert_eq!(bitstream.next_bytes(15), Ok(vec![0; 15]));

        assert_eq!(bitstream.appendable(), None);
        assert_eq!(bitstream.next_bytes(1), Ok(vec![0]));

        assert_eq!(bitstream.appendable(), Some(16));
    }

    #[test]
    fn collects_processed_data() {
        let data = [1, 2, 3, 4];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(7), Some(1));
        assert_eq!(bitstream.next_bits(3), Some(4));

        let mut buffer1 = [0; 10];
        assert_eq!(bitstream.collect(Some(&mut buffer1)), 2);
        assert_eq!(buffer1, [1, 2, 0, 0, 0, 0, 0, 0, 0, 0]);

        let mut buffer2 = [0; 10];
        assert_eq!(bitstream.collect(Some(&mut buffer2)), 0);
        assert_eq!(buffer2, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn collects_processed_data_to_none() {
        let data = [1, 2, 3, 4];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(7), Some(1));
        assert_eq!(bitstream.next_bits(3), Some(4));

        assert_eq!(bitstream.collect(None), 2);
        assert_eq!(bitstream.collect(None), 0);
    }

    #[test]
    fn appends_data_fails_when_not_collected() {
        let data = [1, 2, 3, 4];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(7), Some(1));
        assert_eq!(bitstream.next_bits(3), Some(4));

        let append = [5, 6, 7, 8];

        match bitstream.append(&append) {
            Ok(()) => assert!(true),
            Err(error) => match error {
                BitStreamError::NotCollectedData { collectable } => {
                    assert_eq!(collectable, 2);
                }
                _ => assert!(true),
            },
        }
    }

    #[test]
    fn appends_data_succeeds_when_collected() {
        let data = [1, 2, 3, 4];
        let mut bitstream: BitStreamS<32> = bitstream(&data);

        assert_eq!(bitstream.next_bits(7), Some(1));
        assert_eq!(bitstream.next_bits(3), Some(4));

        let append = [5, 6, 7, 8];

        assert_eq!(bitstream.collect(None), 2);
        assert_eq!(bitstream.append(&append), BitStreamResult::Ok(()));
    }
}
