pub struct Bits([u64; 4]);

pub struct RangeIterator<'a> {
    bits: &'a Bits,
    negation: bool,
    state: usize,
}

impl<'a> Iterator for RangeIterator<'a> {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        let (mut min, mut max) = (0u8, 0u8);
        let state = match u8::try_from(self.state) {
            Err(_) => return None,
            Ok(state) => state,
        };

        for idx in state..=255u8 {
            self.state = idx as usize;

            if self.bits.get(idx) != self.negation {
                if min == 0 {
                    min = idx;
                    max = idx;
                } else {
                    max = idx;
                }
            } else if min > 0 {
                return Some((min, max));
            }
        }

        if min > 0 {
            self.state = 256usize;
            return Some((min, max));
        }

        None
    }
}

impl Bits {
    pub fn new() -> Self {
        Self([0; 4])
    }

    pub fn get(&self, idx: u8) -> bool {
        self.0[idx as usize / 64] & ((1 as u64) << (idx as u64 % 64)) > 0
    }

    pub fn set(&mut self, idx: u8) {
        self.0[idx as usize / 64] |= (1 as u64) << (idx as u64 % 64);
    }

    pub fn iter<'a>(&'a self, negation: bool) -> RangeIterator<'a> {
        RangeIterator{ bits: self, negation: negation, state: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_over_zeroes_positive() {
        let bits = Bits::new();
        let mut iterator = bits.iter(false);

        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_over_zeroes_negative() {
        let bits = Bits::new();
        let mut iterator = bits.iter(true);

        assert_eq!(iterator.next(), Some((1, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_over_single_bit_positive() {
        let mut bits = Bits::new();
        bits.set(17);

        let mut iterator = bits.iter(false);

        assert_eq!(iterator.next(), Some((17, 17)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_over_single_bit_negative() {
        let mut bits = Bits::new();
        bits.set(17);

        let mut iterator = bits.iter(true);

        assert_eq!(iterator.next(), Some((1, 16)));
        assert_eq!(iterator.next(), Some((18, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_over_adjacent_bits_positive() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(18);

        let mut iterator = bits.iter(false);

        assert_eq!(iterator.next(), Some((17, 18)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_over_adjacent_bits_negative() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(18);

        let mut iterator = bits.iter(true);

        assert_eq!(iterator.next(), Some((1, 16)));
        assert_eq!(iterator.next(), Some((19, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_over_disconnected_bits_positive() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(27);

        let mut iterator = bits.iter(false);

        assert_eq!(iterator.next(), Some((17, 17)));
        assert_eq!(iterator.next(), Some((27, 27)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_over_disconnected_bits_negative() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(27);

        let mut iterator = bits.iter(true);

        assert_eq!(iterator.next(), Some((1, 16)));
        assert_eq!(iterator.next(), Some((18, 26)));
        assert_eq!(iterator.next(), Some((28, 255)));
        assert_eq!(iterator.next(), None);
    }
}
