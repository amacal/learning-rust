use std::ptr;

use super::dfa::DFA;
use super::heap::{GuardSegfault, Heap};

pub struct Matrix(*const u16);

struct Builder<const SIZE: usize> {
    heap: Heap<u16, SIZE, GuardSegfault>,
}

impl Matrix {
    pub fn at(data: *const u16) -> Self {
        Self(data)
    }

    pub fn build<const SIZE: usize>(dfa: &DFA) -> Option<Heap<u16, SIZE, GuardSegfault>> {
        Builder::new().build(&dfa)
    }

    pub fn traverse(&self, mut data: *const u8) -> (u16, usize) {
        unsafe {
            let mut state = 2u16;
            let mut best = (0u16, 0usize);

            let start = data;
            let threshold = *self.0.add(1);

            while data != ptr::null() && *data > 0 {
                // next state candidate
                state = *self.0.add(state as usize * 256 + *data as usize);

                // state may be accepting
                if state > threshold {
                    let off = state.wrapping_sub(threshold);
                    let ptr = self.0.add(off as usize);

                    // next best is found in the first row
                    best = (*ptr, data.add(1).offset_from(start) as usize);

                    // next state is found in the second row
                    state = *ptr.add(256);
                }

                // state may be rejecting
                data = if state == 0 { ptr::null() } else { data.add(1) };
            }

            best
        }
    }
}

impl<const SIZE: usize> Builder<SIZE> {
    fn new() -> Self {
        Self { heap: Heap::alloc() }
    }

    pub fn build(mut self, dfa: &DFA) -> Option<Heap<u16, SIZE, GuardSegfault>> {
        let size = dfa.transition_count();
        let high = dfa.transition_at(size - 1).0;

        let threshold = high + 1;
        let mut counter = 2;

        for off in 0..high as usize * 256 + 512 {
            self.heap.set0(0, off);
        }

        for idx in 0..size {
            let (src, via, dst, meta) = dfa.transition_at(idx);

            if meta == 0 {
                for off in via.0..=via.1 {
                    self.heap.set0(dst + 1, src as usize * 256 + off as usize + 256);
                }
            } else {
                for off in via.0..=via.1 {
                    self.heap.set0(counter + threshold, src as usize * 256 + off as usize + 256);
                }

                self.heap.set0(meta, counter);

                if dst != 0 {
                    self.heap.set0(dst + 1, counter + 256);
                }

                counter = counter + 1;
            }
        }

        self.heap.set0(high + 2, 0u16);
        self.heap.set0(threshold, 1u16);

        Some(self.heap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regex::nfa::NFA;
    use crate::regex::rpn::RPN;

    #[test]
    fn handles_building_from_regex_with_one_accepting_state() {
        let regex = b"(st)?op\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let dfa = match DFA::build(nfa) {
            Some(dfa) => dfa,
            None => return assert!(false),
        };

        // 0001 | 6f - 6f | 0002 | 0000
        // 0001 | 73 - 73 | 0003 | 0000
        // 0002 | 70 - 70 | 0000 | 0001
        // 0003 | 74 - 74 | 0004 | 0000
        // 0004 | 6f - 6f | 0005 | 0000
        // 0005 | 70 - 70 | 0000 | 0001

        let heap = match Matrix::build::<4096>(&dfa) {
            Some(heap) => heap,
            None => return assert!(false),
        };

        assert_eq!(heap.get0(0u16), 7);
        assert_eq!(heap.get0(1u16), 6);

        let bytes = heap.as_bytes(7 * 256);

        // first two transitions are just copied
        assert_eq!(bytes[2 * 256 + 0x6f], 3);
        assert_eq!(bytes[2 * 256 + 0x73], 4);

        // next one is accepting
        assert_eq!(bytes[3 * 256 + 0x70], 8);
        assert_eq!(bytes[0 * 256 + 2], 1);
        assert_eq!(bytes[1 * 256 + 2], 0);

        // followed by two transitions copied
        assert_eq!(bytes[4 * 256 + 0x74], 5);
        assert_eq!(bytes[5 * 256 + 0x6f], 6);

        // next one is accepting
        assert_eq!(bytes[6 * 256 + 0x70], 9);
        assert_eq!(bytes[0 * 256 + 3], 1);
        assert_eq!(bytes[1 * 256 + 3], 0);
    }

    #[test]
    fn handles_traversing_from_regex_with_one_accepting_state() {
        let regex = b"(st)?op\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let dfa = match DFA::build(nfa) {
            Some(dfa) => dfa,
            None => return assert!(false),
        };

        let heap = match Matrix::build::<4096>(&dfa) {
            Some(heap) => heap,
            None => return assert!(false),
        };

        let len = heap.get0(0u16);
        let mem = heap.as_bytes(len as usize);
        let matrix = Matrix::at(mem.as_ptr());

        // fully accepted
        assert_eq!(matrix.traverse(b"stop\0".as_ptr()), (1, 4));
        assert_eq!(matrix.traverse(b"op\0".as_ptr()), (1, 2));

        // not accepted
        assert_eq!(matrix.traverse(b"top\0".as_ptr()), (0, 0));
        assert_eq!(matrix.traverse(b"start\0".as_ptr()), (0, 0));
    }

    #[test]
    fn handles_building_from_regex_with_two_accepting_states() {
        let regex = b"(start#x)(stop)?#y\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let dfa = match DFA::build(nfa) {
            Some(dfa) => dfa,
            None => return assert!(false),
        };

        // 0001 | 73 - 73 | 0002 | 0000
        // 0002 | 74 - 74 | 0003 | 0000
        // 0003 | 61 - 61 | 0004 | 0000
        // 0004 | 72 - 72 | 0005 | 0000
        // 0005 | 74 - 74 | 0006 | 0078
        // 0006 | 73 - 73 | 0007 | 0000
        // 0007 | 74 - 74 | 0008 | 0000
        // 0008 | 6f - 6f | 0009 | 0000
        // 0009 | 70 - 70 | 0000 | 0079

        let heap = match Matrix::build::<8192>(&dfa) {
            Some(heap) => heap,
            None => return assert!(false),
        };

        assert_eq!(heap.get0(0u16), 11);
        assert_eq!(heap.get0(1u16), 10);

        let bytes = heap.as_bytes(11 * 256);

        // first four transitions are just copied
        assert_eq!(bytes[2 * 256 + 0x73], 3);
        assert_eq!(bytes[3 * 256 + 0x74], 4);
        assert_eq!(bytes[4 * 256 + 0x61], 5);
        assert_eq!(bytes[5 * 256 + 0x72], 6);

        // next one is accepting
        assert_eq!(bytes[6 * 256 + 0x74], 12);
        assert_eq!(bytes[0 * 256 + 2], 0x78);
        assert_eq!(bytes[1 * 256 + 2], 7);

        // followed by three transitions copied
        assert_eq!(bytes[7 * 256 + 0x73], 8);
        assert_eq!(bytes[8 * 256 + 0x74], 9);
        assert_eq!(bytes[9 * 256 + 0x6f], 10);

        // next one is accepting
        assert_eq!(bytes[10 * 256 + 0x70], 13);
        assert_eq!(bytes[0 * 256 + 3], 0x79);
        assert_eq!(bytes[1 * 256 + 3], 0);
    }

    #[test]
    fn handles_traversing_from_regex_with_two_accepting_states() {
        let regex = b"(start#x)(stop)?#y\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let dfa = match DFA::build(nfa) {
            Some(dfa) => dfa,
            None => return assert!(false),
        };

        let heap = match Matrix::build::<8192>(&dfa) {
            Some(heap) => heap,
            None => return assert!(false),
        };

        let len = heap.get0(0u16);
        let mem = heap.as_bytes(len as usize);
        let matrix = Matrix::at(mem.as_ptr());

        // fully accepted
        assert_eq!(matrix.traverse(b"start\0".as_ptr()), (0x78, 5));
        assert_eq!(matrix.traverse(b"startstop\0".as_ptr()), (0x79, 9));

        // not accepted
        assert_eq!(matrix.traverse(b"topp\0".as_ptr()), (0, 0));
        assert_eq!(matrix.traverse(b"star\0".as_ptr()), (0, 0));
    }

}
