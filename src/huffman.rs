use std::fmt::Display;

use crate::bitstream::BitStream;

#[derive(Debug, PartialEq, Default, Clone, Copy)]
pub struct HuffmanCode {
    pub bits: u16,
    pub length: usize,
}

pub struct HuffmanTable<const MAX_BITS: usize, const MAX_SYMBOLS: usize> {
    shortest: usize,              // the shortest code in the table
    counts: [u16; MAX_BITS],      // the array of counts per each length
    symbols: [u16; MAX_SYMBOLS],  // the array of symbols
}

pub type HuffmanResult<T> = Result<T, HuffmanError>;

#[derive(PartialEq, Debug, thiserror::Error)]
pub enum HuffmanError {
    #[error("Not enough data")]
    NotEnoughData,

    #[error("Invalid symbol")]
    InvalidSymbol,
}

impl HuffmanError {
    fn raise_not_enough_data<T>() -> HuffmanResult<T> {
        Err(HuffmanError::NotEnoughData)
    }

    fn raise_invalid_symbol<T>() -> HuffmanResult<T> {
        Err(HuffmanError::InvalidSymbol)
    }
}

impl<const MAX_BITS: usize, const MAX_SYMBOLS: usize> HuffmanTable<MAX_BITS, MAX_SYMBOLS> {
    pub fn new(lengths: [u16; MAX_SYMBOLS]) -> Option<Self> {
        let mut counts = [0; MAX_BITS];
        let mut symbols = [0; MAX_SYMBOLS];
        let mut offsets = [0; MAX_BITS];
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

        if shortest == 0 {
            return None;
        }

        Some(Self {
            shortest: shortest,
            counts: counts,
            symbols: symbols,
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

    pub fn decode(&self, bits: &mut impl BitStream) -> HuffmanResult<u16> {
        let mut first: u16 = 0;
        let mut code: u16 = 0;
        let mut offset: u16 = 0;

        for _ in 1..self.shortest {
            code |= match bits.next_bit() {
                Some(bit) => bit as u16,
                None => return HuffmanError::raise_not_enough_data(),
            };

            code <<= 1;
        }

        for &count in self.counts[self.shortest..].iter() {
            code |= match bits.next_bit() {
                Some(bit) => bit as u16,
                None => return HuffmanError::raise_not_enough_data(),
            };

            if code < first + count {
                let index = offset as usize + (code - first) as usize;
                let symbol = match self.symbols.get(index) {
                    Some(&value) => value,
                    None => return HuffmanError::raise_invalid_symbol(),
                };

                return Ok(symbol);
            }

            offset += count;
            first += count;

            first <<= 1;
            code <<= 1;
        }

        HuffmanError::raise_invalid_symbol()
    }
}

impl HuffmanCode {
    pub fn new(bits: u16, length: usize) -> Self {
        Self {
            bits: bits,
            length: length,
        }
    }
}

impl Display for HuffmanCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:0width$b}", self.bits, width = self.length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitstream::BitStreamBytewise;

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
        let table: HuffmanTable<4, 5> = HuffmanTable::new([0, 2, 3, 1, 3]).unwrap();

        assert_eq!(table.shortest, 1);
        assert_eq!(table.counts, [0, 1, 1, 2]);
        assert_eq!(table.symbols, [3, 1, 2, 4, 0]);
    }

    #[test]
    fn creates_huffman_table_fails() {
        let table: Option<HuffmanTable<4, 5>> = HuffmanTable::new([0, 0, 0, 0, 0]);

        assert!(table.is_none());
    }

    #[test]
    fn lists_huffman_table() {
        let table: HuffmanTable<4, 5> = HuffmanTable::new([0, 2, 3, 1, 3]).unwrap();
        let codes = table.list();

        assert_eq!(codes[0], HuffmanCode::default());
        assert_eq!(codes[1], HuffmanCode::new(0b10, 2));
        assert_eq!(codes[2], HuffmanCode::new(0b110, 3));
        assert_eq!(codes[3], HuffmanCode::new(0b0, 1));
        assert_eq!(codes[4], HuffmanCode::new(0b111, 3));
    }

    #[test]
    fn decodes_using_huffman_table() {
        let table: HuffmanTable<4, 5> = HuffmanTable::new([0, 2, 3, 1, 3]).unwrap();
        let mut bitstream: BitStreamBytewise<2> = bitstream(&[0b11011010, 0b00000001]);

        assert_eq!(table.decode(&mut bitstream).unwrap(), 3);
        assert_eq!(table.decode(&mut bitstream).unwrap(), 1);
        assert_eq!(table.decode(&mut bitstream).unwrap(), 2);
        assert_eq!(table.decode(&mut bitstream).unwrap(), 4);
        assert_eq!(table.decode(&mut bitstream).unwrap(), 3);
    }

    #[test]
    fn decodes_using_huffman_table_failing() {
        let table: HuffmanTable<4, 5> = HuffmanTable::new([0, 2, 3, 1, 0]).unwrap();
        let mut bitstream: BitStreamBytewise<1> = bitstream(&[0b111]);

        match table.decode(&mut bitstream) {
            Ok(_) => assert!(false),
            Err(_) => assert!(true,),
        };
    }
}
