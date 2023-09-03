use crate::bitstream::BitStream;

pub struct HuffmanTable<const MAX_BITS: usize, const MAX_SYMBOLS: usize> {
    shortest: usize,
    counts: [u16; MAX_BITS],
    symbols: [u16; MAX_SYMBOLS],
}

impl<const MAX_BITS: usize, const MAX_SYMBOLS: usize> HuffmanTable<MAX_BITS, MAX_SYMBOLS>
{
    pub fn new(lengths: [u16; MAX_SYMBOLS]) -> Self {
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

        Self {
            shortest: shortest,
            counts: counts,
            symbols: symbols,
        }
    }

    pub fn list(&self) -> Vec<String> {
        let mut result = Vec::with_capacity(self.symbols.len());
        let mut bits = 0;

        for (length, &count) in self.counts.iter().enumerate() {
            if count > 0 {
                bits <<= 1;
            }

            for _ in 0..count {
                result.push(format!("{:0width$b} at {}", bits, length, width=length).to_string());
                bits += 1;
            }
        }

        return result
    }

    pub fn decode(&self, bits: &mut BitStream) -> Option<u16> {
        let mut first: u16 = 0;
        let mut code: u16 = 0;
        let mut offset: u16 = 0;

        for _ in 1..self.shortest {
            code |= match bits.next_bit() {
                Some(bit) => bit as u16,
                None => return None,
            };

            code <<= 1;
        }

        for &count in self.counts[self.shortest..].iter() {
            code |= match bits.next_bit() {
                Some(bit) => bit as u16,
                None => return None,
            };

            if code < first + count {
                let index = offset as usize + (code - first) as usize;
                let symbol = match self.symbols.get(index) {
                    Some(&value) => value,
                    None => return None,
                };

                return Some(symbol);
            }

            offset += count;
            first += count;

            first <<= 1;
            code <<= 1;
        }

        None
    }
}
