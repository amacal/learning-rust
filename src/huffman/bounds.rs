use crate::bitstream::BitReader;
use crate::huffman::header::{HuffmanCode, HuffmanDecoder, HuffmanError, HuffmanResult};

pub struct HuffmanTableBounds<const MAX_BITS: usize, const MAX_SYMBOLS: usize> {
    counts: [u16; MAX_BITS],     // the array of counts per each length
    lower: [u16; MAX_BITS],      // the array of lower values at each length
    upper: [u16; MAX_BITS],      // the array of upper values at each length
    offsets: [u16; MAX_BITS],    // the array of symbols' offsets at each length
    symbols: [u16; MAX_SYMBOLS], // the array of symbols
}

impl<const MAX_BITS: usize, const MAX_SYMBOLS: usize> HuffmanTableBounds<MAX_BITS, MAX_SYMBOLS> {
    pub fn new(lengths: [u16; MAX_SYMBOLS]) -> Option<Self> {
        let mut counts = [0; MAX_BITS];
        let mut symbols = [0; MAX_SYMBOLS];
        let mut offsets = [0; MAX_BITS];
        let mut lower = [0; MAX_BITS];
        let mut upper = [0; MAX_BITS];
        let mut shortest = 0;

        for index in 0..MAX_SYMBOLS {
            if lengths[index] > 0 {
                counts[lengths[index] as usize] += 1;
            }
        }

        for index in 1..MAX_BITS {
            offsets[index] = offsets[index - 1] + counts[index - 1];

            if shortest == 0 && counts[index] > 0 {
                shortest = index;
            }
        }

        for index in 0..MAX_SYMBOLS {
            if lengths[index] > 0 {
                symbols[offsets[lengths[index] as usize] as usize] = index as u16;
                offsets[lengths[index] as usize] += 1;
            }
        }

        let mut first = 0;
        let mut offset = 0;

        for index in 0..MAX_BITS {
            lower[index] = first;
            offsets[index] = offset;
            upper[index] = first + counts[index];

            offset += counts[index];
            first += counts[index];
            first <<= 1;
        }

        if shortest == 0 {
            return None;
        }

        Some(Self {
            counts: counts,
            symbols: symbols,
            lower: lower,
            upper: upper,
            offsets: offsets,
        })
    }

    pub fn list(&self) -> [HuffmanCode; MAX_SYMBOLS] {
        let mut bits = 0;
        let mut offset = 0;
        let mut codes = [HuffmanCode::default(); MAX_SYMBOLS];

        for (length, &count) in self.counts.iter().enumerate() {
            if count > 0 {
                bits <<= 1;
            }

            for _ in 0..count {
                codes[self.symbols[offset] as usize] = HuffmanCode::new(bits, length);

                bits += 1;
                offset += 1;
            }
        }

        return codes;
    }
}

impl<const MAX_BITS: usize, const MAX_SYMBOLS: usize> HuffmanDecoder for HuffmanTableBounds<MAX_BITS, MAX_SYMBOLS> {
    fn decode(&self, bits: &mut impl BitReader) -> HuffmanResult<u16> {
        let mut code: u16 = 0;

        for length in 1..MAX_BITS {
            code |= match bits.next_bit() {
                Some(bit) => bit as u16,
                None => return HuffmanError::raise_not_enough_data(),
            };

            unsafe {
                let lower = *self.lower.get_unchecked(length);
                let upper = *self.upper.get_unchecked(length);

                if code >= lower && code < upper {
                    let offset = *self.offsets.get_unchecked(length);
                    let index = offset as usize + (code - lower) as usize;

                    let symbol = match self.symbols.get(index) {
                        Some(&value) => value,
                        None => return HuffmanError::raise_invalid_symbol(code),
                    };

                    return Ok(symbol);
                }
            }

            code <<= 1;
        }

        HuffmanError::raise_invalid_symbol(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitstream::{BitStream, BitStreamBytewise, BitStreamExt};

    fn bitstream<const T: usize>(data: &[u8]) -> BitStreamBytewise<T> {
        let mut bitstream = BitStreamBytewise::new();
        bitstream.append(data).unwrap();
        bitstream
    }

    #[test]
    fn formats_huffman_code() {
        assert_eq!(format!("{}", HuffmanCode::new(0b101, 4)), "0101");
    }

    #[test]
    fn creates_huffman_table() {
        let table: HuffmanTableBounds<4, 5> = HuffmanTableBounds::new([0, 2, 3, 1, 3]).unwrap();

        assert_eq!(table.counts, [0, 1, 1, 2]);
        assert_eq!(table.symbols, [3, 1, 2, 4, 0]);
    }

    #[test]
    fn creates_huffman_table_fails() {
        let table: Option<HuffmanTableBounds<4, 5>> = HuffmanTableBounds::new([0, 0, 0, 0, 0]);

        assert!(table.is_none());
    }

    #[test]
    fn lists_huffman_table() {
        let table: HuffmanTableBounds<4, 5> = HuffmanTableBounds::new([0, 2, 3, 1, 3]).unwrap();
        let codes = table.list();

        assert_eq!(codes[0], HuffmanCode::default());
        assert_eq!(codes[1], HuffmanCode::new(0b10, 2));
        assert_eq!(codes[2], HuffmanCode::new(0b110, 3));
        assert_eq!(codes[3], HuffmanCode::new(0b0, 1));
        assert_eq!(codes[4], HuffmanCode::new(0b111, 3));
    }

    #[test]
    fn decodes_using_huffman_table() {
        let table: HuffmanTableBounds<4, 5> = HuffmanTableBounds::new([0, 2, 3, 1, 3]).unwrap();
        let mut bitstream: BitStreamBytewise<2> = bitstream(&[0b11011010, 0b00000001]);
        let mut reader = bitstream.as_checked();

        assert_eq!(table.decode(&mut reader).unwrap(), 3);
        assert_eq!(table.decode(&mut reader).unwrap(), 1);
        assert_eq!(table.decode(&mut reader).unwrap(), 2);
        assert_eq!(table.decode(&mut reader).unwrap(), 4);
        assert_eq!(table.decode(&mut reader).unwrap(), 3);
    }

    #[test]
    fn decodes_using_huffman_table_failing() {
        let table: HuffmanTableBounds<4, 5> = HuffmanTableBounds::new([0, 2, 3, 1, 0]).unwrap();
        let mut bitstream: BitStreamBytewise<1> = bitstream(&[0b111]);
        let mut reader = bitstream.as_checked();

        match table.decode(&mut reader) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true,),
        };
    }
}
