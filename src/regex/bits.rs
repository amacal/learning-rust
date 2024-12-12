#[derive(Debug)]
pub struct Bits([u64; 4]);

pub struct AreaIterator<'a> {
    bits: &'a Bits,
    negation: bool,
    state: usize,
}

pub struct EdgeIterator<'a> {
    bits: &'a Bits,
    state: usize,
}

impl<'a> Iterator for AreaIterator<'a> {
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

impl<'a> Iterator for EdgeIterator<'a> {
    type Item = (u8, u8);

    fn next(&mut self) -> Option<Self::Item> {
        let state = match u8::try_from(self.state) {
            Err(_) => return None,
            Ok(state) => state,
        };

        for idx in state + 1..255u8 {
            self.state = idx as usize;

            if self.bits.get(idx) {
                return Some((state, idx - 1));
            }
        }

        if self.bits.get(255) {
            self.state = 256usize;
            return Some((state, 255));
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

    pub fn area<'a>(&'a self, negation: bool) -> AreaIterator<'a> {
        AreaIterator { bits: self, negation: negation, state: 0 }
    }

    pub fn edge<'a>(&'a self) -> EdgeIterator<'a> {
        for idx in 1..=255u8 {
            if self.get(idx) {
                return EdgeIterator { bits: self, state: idx.into() };
            }
        }

        EdgeIterator { bits: self, state: 256 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_area_over_zeroes_positive() {
        let bits = Bits::new();
        let mut iterator = bits.area(false);

        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_area_over_zeroes_negative() {
        let bits = Bits::new();
        let mut iterator = bits.area(true);

        assert_eq!(iterator.next(), Some((1, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_area_over_single_bit_positive() {
        let mut bits = Bits::new();
        bits.set(17);

        let mut iterator = bits.area(false);

        assert_eq!(iterator.next(), Some((17, 17)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_area_over_single_bit_negative() {
        let mut bits = Bits::new();
        bits.set(17);

        let mut iterator = bits.area(true);

        assert_eq!(iterator.next(), Some((1, 16)));
        assert_eq!(iterator.next(), Some((18, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_area_over_adjacent_bits_positive() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(18);

        let mut iterator = bits.area(false);

        assert_eq!(iterator.next(), Some((17, 18)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_area_over_adjacent_bits_negative() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(18);

        let mut iterator = bits.area(true);

        assert_eq!(iterator.next(), Some((1, 16)));
        assert_eq!(iterator.next(), Some((19, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_area_over_disconnected_bits_positive() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(27);

        let mut iterator = bits.area(false);

        assert_eq!(iterator.next(), Some((17, 17)));
        assert_eq!(iterator.next(), Some((27, 27)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_area_over_disconnected_bits_negative() {
        let mut bits = Bits::new();

        bits.set(17);
        bits.set(27);

        let mut iterator = bits.area(true);

        assert_eq!(iterator.next(), Some((1, 16)));
        assert_eq!(iterator.next(), Some((18, 26)));
        assert_eq!(iterator.next(), Some((28, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_edge_over_zeroes() {
        let bits = Bits::new();
        let mut iterator = bits.edge();

        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_edge_over_single_region() {
        let mut bits = Bits::new();

        bits.set(20);
        bits.set(41);

        let mut iterator = bits.edge();

        assert_eq!(iterator.next(), Some((20, 40)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_edge_over_single_region_everything() {
        let mut bits = Bits::new();

        bits.set(1);
        bits.set(255);

        let mut iterator = bits.edge();

        assert_eq!(iterator.next(), Some((1, 255)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_edge_over_two_regions() {
        let mut bits = Bits::new();

        bits.set(20);
        bits.set(41);
        bits.set(60);
        bits.set(81);

        let mut iterator = bits.edge();

        assert_eq!(iterator.next(), Some((20, 40)));
        assert_eq!(iterator.next(), Some((41, 59)));
        assert_eq!(iterator.next(), Some((60, 80)));
        assert_eq!(iterator.next(), None);
    }

    #[test]
    fn iterates_edge_over_two_regions_overlapping() {
        let mut bits = Bits::new();

        bits.set(20);
        bits.set(61);
        bits.set(40);
        bits.set(81);

        let mut iterator = bits.edge();

        assert_eq!(iterator.next(), Some((20, 39)));
        assert_eq!(iterator.next(), Some((40, 60)));
        assert_eq!(iterator.next(), Some((61, 80)));
        assert_eq!(iterator.next(), None);
    }
}
