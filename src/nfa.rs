use crate::array::Array;
use crate::array::StackLike;

use super::graph::*;
use super::heap::*;
use super::list::*;
use super::rpn::*;

pub struct NFA {
    counter: u16,
    transitions: Graph<4096, GuardSegfault>,
    epsilons: Collection<4096, GuardSegfault>,
}

impl NFA {
    pub fn new() -> Self {
        Self {
            counter: 0,
            transitions: Graph::new(),
            epsilons: Collection::new(),
        }
    }

    fn from(transitions: Graph<4096, GuardSegfault>, epsilons: Collection<4096, GuardSegfault>) -> Self {
        Self {
            counter: 0,
            transitions: transitions,
            epsilons: epsilons,
        }
    }

    pub fn next(&mut self) -> u16 {
        self.counter = self.counter.wrapping_add(1);
        self.counter.wrapping_sub(1)
    }

    pub fn transition_count(&self) -> u16 {
        self.transitions.graph_count()
    }

    pub fn transition_at(&self, idx: u16) -> (u16, (u8, u8), u16, u16) {
        self.transitions.graph_at(idx)
    }

    pub fn transition_find(&self, src: u16, via: u8) -> Option<(u16, u16)> {
        self.transitions.graph_find(src, via)
    }

    pub fn transition_find_all(&self, src: u16) -> Option<(u16, u16)> {
        self.transitions.graph_find_all(src)
    }

    pub fn transition_inc(&mut self) -> u16 {
        self.transitions.graph_inc()
    }

    pub fn transition_add(&mut self, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        self.transitions.graph_add(src, via, dst, metadata);
    }

    pub fn transition_set(&mut self, idx: u16, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        self.transitions.graph_set(idx, src, via, dst, metadata);
    }

    fn transition_sort(&mut self) {
        self.transitions.graph_sort();
    }

    pub fn epsilon_new(&mut self) -> u16 {
        self.epsilons.list_push_head()
    }

    pub fn epsilon_count(&self) -> u16 {
        self.epsilons.list_count()
    }

    pub fn epsilon_items_resize(&mut self, idx: u16, size: u16) {
        self.epsilons.list_items_resize(idx, size)
    }

    pub fn epsilon_items_add(&mut self, idx: u16, item: u16) {
        self.epsilons.list_items_add(idx, item)
    }

    pub fn epsilon_items_set(&mut self, idx: u16, off: u16, item: u16) {
        self.epsilons.list_items_set(idx, off, item)
    }

    pub fn epsilon_items_get(&self, idx: u16, off: u16) -> u16 {
        self.epsilons.list_items_get(idx, off)
    }

    fn epsilon_items_sort(&mut self, idx: u16) {
        self.epsilons.list_items_sort(idx)
    }

    fn epsilon_items_distinct(&mut self, idx: u16) {
        self.epsilons.list_items_distinct(idx)
    }

    pub fn epsilon_items_count(&self, idx: u16) -> u16 {
        self.epsilons.list_items_count(idx)
    }

    pub fn print(&self) {
        for idx in 0..self.transition_count() {
            let transition = self.transition_at(idx);
            print!(
                "{:04x} | {:02x} - {:02x} | {:04x} | ",
                transition.0, transition.1 .0, transition.1 .1, transition.3
            );

            if transition.1 .0 > 0 {
                println!("{:04x}", transition.2);
            } else {
                print!("{:04x} -> ", transition.2);

                for off in 0..self.epsilon_items_count(transition.2) {
                    print!("{:04x} ", self.epsilon_items_get(transition.2, off))
                }

                println!();
            }
        }

        println!();
    }
}

struct Indices {}
impl StackLike for Indices {}

struct Builder {
    counter: u16,
    transitions: Graph<4096, GuardSegfault>,
    epsilons: Collection<4096, GuardSegfault>,
    stack: Array<Indices, u16, 4096, GuardSegfault>,
}

impl Builder {
    fn new() -> Self {
        Self {
            counter: 0,
            transitions: Graph::new(),
            epsilons: Collection::new(),
            stack: Array::new(),
        }
    }

    fn next(&mut self) -> u16 {
        self.counter = self.counter.wrapping_add(1);
        self.counter.wrapping_sub(1)
    }

    fn build<const SIZE: usize>(mut self, rpn: RPN<SIZE>) -> Option<NFA> {
        let mut idx = 0u16;

        let start_state = self.next();
        let end_state = self.next();

        let start_entry = self.transitions.graph_inc();
        let end_entry = self.transitions.graph_inc();

        let start_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_resize(start_epsilons, 1);

        let end_epsilons = self.epsilons.list_push_head();
        self.transitions.graph_set(end_entry, end_state, (0, 0), end_epsilons, 0x00);
        self.transitions.graph_set(start_entry, start_state, (0, 0), start_epsilons, 0x00);

        while idx < rpn.size() {
            match rpn.at(idx) {
                b'l' => {
                    let index = idx.wrapping_add(1);
                    let count = rpn.at(index).wrapping_sub(b'0') as u16;

                    let zero_state = self.next();
                    let first = self.next();
                    let mut last = first;

                    let zero_epsilons = self.epsilons.list_push_head();
                    self.epsilons.list_items_add(zero_epsilons, first);
                    self.transitions.graph_add(zero_state, (0, 0), zero_epsilons, 0x02);


                    for idx in index.wrapping_add(1)..=index.wrapping_add(count) {
                        let inside = idx > 0;
                        let val = rpn.at(idx as u16);

                        let next = self.next();
                        let meta = if inside { 0x02 } else { 0x00 };
                        //meta += if accepting && !inside { 0x01 } else { 0x00 };

                        self.transitions.graph_add(last, (val, val), next, meta);
                        last = next;
                    }

                    let complete_epsilons = self.epsilons.list_push_head();
                    self.epsilons.list_items_resize(complete_epsilons, 1);

                    self.transitions.graph_add(last, (0, 0), complete_epsilons, 0x00);
                    idx = index.wrapping_add(count).wrapping_add(1);

                    self.stack.stack_push(zero_state);
                    self.stack.stack_push(complete_epsilons);
                }
                b'|' => {
                    let right_last_epsilons = self.stack.stack_pop();
                    let right_first_state = self.stack.stack_pop();

                    let left_last_epsilons = self.stack.stack_pop();
                    let left_first_state = self.stack.stack_pop();

                    let start_state = self.next();
                    let end_state = self.next();

                    let start_epsilons = self.epsilons.list_push_head();
                    self.epsilons.list_items_add(start_epsilons, left_first_state);
                    self.epsilons.list_items_add(start_epsilons, right_first_state);

                    self.epsilons.list_items_set(left_last_epsilons, 0, end_state);
                    self.epsilons.list_items_set(right_last_epsilons, 0, end_state);

                    idx = idx.wrapping_add(1);
                    self.transitions.graph_add(start_state, (0, 0), start_epsilons, 0x00);

                    let end_epsilons = self.epsilons.list_push_head();
                    self.transitions.graph_add(end_state, (0, 0), end_epsilons, 0x00);
                    self.epsilons.list_items_resize(end_epsilons, 1);

                    self.stack.stack_push(start_state);
                    self.stack.stack_push(end_epsilons);
                }
                _ => break,
            }
        }

        let last_epsilons = self.stack.stack_pop();
        let first_state = self.stack.stack_pop();

        self.epsilons.list_items_set(start_epsilons, 0, first_state);
        self.epsilons.list_items_set(last_epsilons, 0, end_state);

        Some(NFA::from(self.transitions, self.epsilons))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_converting_rpn_seq_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l5start");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 0002
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0002 | 0009 -> 0003
        // 0003 | 73 - 73 | 0002 | 0004
        // 0004 | 74 - 74 | 0002 | 0005
        // 0005 | 61 - 61 | 0002 | 0006
        // 0006 | 72 - 72 | 0002 | 0007
        // 0007 | 74 - 74 | 0002 | 0008
        // 0008 | 00 - 00 | 0000 | 000e -> 0001

        assert_eq!(nfa.transition_count(), 9);
        assert_eq!(nfa.epsilon_count(), 4);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x02)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x02)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x02)));
        assert_eq!(nfa.transition_find(5, b'a'), Some((6, 0x02)));
        assert_eq!(nfa.transition_find(6, b'r'), Some((7, 0x02)));
        // assert_eq!(nfa.transition_find(7, b't'), Some((8, 0x01))); todo

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_either_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l5startl4stop|");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 000f
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0002 | 0009 -> 0003
        // 0003 | 73 - 73 | 0002 | 0004
        // 0004 | 74 - 74 | 0002 | 0005
        // 0005 | 61 - 61 | 0002 | 0006
        // 0006 | 72 - 72 | 0002 | 0007
        // 0007 | 74 - 74 | 0002 | 0008
        // 0008 | 00 - 00 | 0000 | 000e -> 0010
        // 0009 | 00 - 00 | 0002 | 0013 -> 000a
        // 000a | 73 - 73 | 0002 | 000b
        // 000b | 74 - 74 | 0002 | 000c
        // 000c | 6f - 6f | 0002 | 000d
        // 000d | 70 - 70 | 0002 | 000e
        // 000e | 00 - 00 | 0000 | 0018 -> 0010
        // 000f | 00 - 00 | 0000 | 001d -> 0002 0009
        // 0010 | 00 - 00 | 0000 | 0023 -> 0001

        assert_eq!(nfa.transition_count(), 17);
        assert_eq!(nfa.epsilon_count(), 8);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 15);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon, beginning of literal
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x02)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x02)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x02)));
        assert_eq!(nfa.transition_find(5, b'a'), Some((6, 0x02)));
        assert_eq!(nfa.transition_find(6, b'r'), Some((7, 0x02)));
        // assert_eq!(nfa.transition_find(7, b't'), Some((8, 0x01))); todo

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 16);

        // points at epsilon, beginning of literal
        assert_eq!(nfa.transition_find(9, 0), Some((0x13, 0x02)));
        assert_eq!(nfa.epsilon_items_count(0x13), 1);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 10);

        // connects 'stop' literal
        assert_eq!(nfa.transition_find(10, b's'), Some((11, 0x02)));
        assert_eq!(nfa.transition_find(11, b't'), Some((12, 0x02)));
        assert_eq!(nfa.transition_find(12, b'o'), Some((13, 0x02)));
        // assert_eq!(nfa.transition_find(13, b'p'), Some((14, 0x01))); todo

        // points at epsilon
        assert_eq!(nfa.transition_find(14, 0), Some((0x18, 0)));
        assert_eq!(nfa.epsilon_items_count(0x18), 1);
        assert_eq!(nfa.epsilon_items_get(0x18, 0), 16);

        // points at epsilon
        assert_eq!(nfa.transition_find(15, 0), Some((0x1d, 0)));
        assert_eq!(nfa.epsilon_items_count(0x1d), 2);
        assert_eq!(nfa.epsilon_items_get(0x1d, 0), 2);
        assert_eq!(nfa.epsilon_items_get(0x1d, 1), 9);

        // points at epsilon
        assert_eq!(nfa.transition_find(16, 0), Some((0x23, 0)));
        assert_eq!(nfa.epsilon_items_count(0x23), 1);
        assert_eq!(nfa.epsilon_items_get(0x23, 0), 1);
    }

}
