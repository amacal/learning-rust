use super::array::*;
use super::graph::*;
use super::heap::*;
use super::list::*;
use super::rpn::*;

pub struct NFA {
    transitions: Graph<8192, GuardSegfault>,
    epsilons: Collection<8192, GuardSegfault>,
}

impl NFA {
    fn from(transitions: Graph<8192, GuardSegfault>, epsilons: Collection<8192, GuardSegfault>) -> Self {
        Self {
            transitions: transitions,
            epsilons: epsilons,
        }
    }

    pub fn build<const SIZE: usize>(rpn: RPN<SIZE>) -> Option<Self> {
        Builder::new().build(rpn)
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

    pub fn epsilon_items_get(&self, idx: u16, off: u16) -> u16 {
        self.epsilons.list_items_get(idx, off)
    }

    pub fn epsilon_items_count(&self, idx: u16) -> u16 {
        self.epsilons.list_items_count(idx)
    }

    pub fn print(&self) {
        for idx in 0..self.transition_count() {
            let transition = self.transitions.graph_at(idx);
            print!("{:04x} | {:02x} - {:02x} | {:04x} | ", transition.0, transition.1 .0, transition.1 .1, transition.3);

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

struct Collapsed {}
impl StackLike for Collapsed {}

struct Accepting {}
impl StackLike for Accepting {}

struct Builder {
    counter: u16,
    transitions: Graph<8192, GuardSegfault>,
    epsilons: Collection<8192, GuardSegfault>,
    collapsed: Array<Collapsed, u16, 4096, GuardSegfault>,
    accepting: Array<Accepting, u16, 4096, GuardSegfault>,
}

impl Builder {
    fn new() -> Self {
        Self {
            counter: 0,
            transitions: Graph::new(),
            epsilons: Collection::new(),
            collapsed: Array::new(),
            accepting: Array::new(),
        }
    }

    fn next(&mut self) -> u16 {
        self.counter += 1;
        self.counter - 1
    }

    fn handle_literal<const SIZE: usize>(&mut self, rpn: &RPN<SIZE>, idx: u16) -> u16 {
        let index = idx + 1;
        let count = rpn.at(index) - b'0';

        let zero_state = self.next();
        let first_state = self.next();
        let mut last = first_state;

        let upper_limit = index + count as u16;
        let zero_epsilons = self.epsilons.list_push_head();

        self.epsilons.list_items_add(zero_epsilons, first_state);
        self.transitions.graph_add(zero_state, (0, 0), zero_epsilons, 0x0200);

        for idx in index+1..=upper_limit {
            let inside = idx < upper_limit;
            let val = rpn.at(idx);

            let next = self.next();
            let meta = if inside { 0x0200 } else { 0x00 };

            let idx = self.transitions.graph_inc();
            self.transitions.graph_set(idx, last, (val, val), next, meta);

            if !inside {
                self.accepting.stack_push_front(idx);
                self.accepting.stack_push_front(0);
            }

            last = next;
        }

        let complete_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_resize(complete_epsilons, 1);

        self.transitions.graph_add(last, (0, 0), complete_epsilons, 0x00);

        self.collapsed.stack_push_front(zero_state);
        self.collapsed.stack_push_front(complete_epsilons);

        index + count as u16 + 1
    }

    fn handle_positive_class<const SIZE: usize>(&mut self, rpn: &RPN<SIZE>, idx: u16) -> u16 {
        let low = rpn.at(idx + 1);
        let high = rpn.at(idx + 2);

        let zero_state = self.next();
        let first_state = self.next();

        let last_state = self.next();
        let zero_epsilons = self.epsilons.list_push_head();

        self.epsilons.list_items_add(zero_epsilons, first_state);
        self.transitions.graph_add(zero_state, (0, 0), zero_epsilons, 0x0200);

        let complete_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_resize(complete_epsilons, 1);

        let first_state_idx = self.transitions.graph_inc();
        self.transitions.graph_set(first_state_idx, first_state, (low, high), last_state, 0x00);
        self.transitions.graph_add(last_state, (0, 0), complete_epsilons, 0x00);

        self.accepting.stack_push_front(first_state_idx);
        self.accepting.stack_push_front(0);

        self.collapsed.stack_push_front(zero_state);
        self.collapsed.stack_push_front(complete_epsilons);

        idx + 3
    }

    fn handle_alternation(&mut self, idx: u16) -> u16 {
        let right_last_epsilons = self.collapsed.stack_pop_front();
        let right_first_state = self.collapsed.stack_pop_front();

        let left_last_epsilons = self.collapsed.stack_pop_front();
        let left_first_state = self.collapsed.stack_pop_front();

        let start_state = self.next();
        let end_state = self.next();

        let start_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_add(start_epsilons, left_first_state);
        self.epsilons.list_items_add(start_epsilons, right_first_state);

        self.epsilons.list_items_set(left_last_epsilons, 0, end_state);
        self.epsilons.list_items_set(right_last_epsilons, 0, end_state);

        self.transitions.graph_add(start_state, (0, 0), start_epsilons, 0x00);

        let end_epsilons = self.epsilons.list_push_head();
        self.transitions.graph_add(end_state, (0, 0), end_epsilons, 0x00);
        self.epsilons.list_items_resize(end_epsilons, 1);

        self.collapsed.stack_push_front(start_state);
        self.collapsed.stack_push_front(end_epsilons);
        self.accepting.stack_push_front(b'|' as u16);

        idx + 1
    }

    fn handle_concatenation(&mut self, idx: u16) -> u16 {
        let right_last_epsilons = self.collapsed.stack_pop_front();
        let right_first_state = self.collapsed.stack_pop_front();

        let left_last_epsilons = self.collapsed.stack_pop_front();
        let left_first_state = self.collapsed.stack_pop_front();

        let start_state = self.next();
        let continue_state = self.next();
        let end_state = self.next();

        let start_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_add(start_epsilons, left_first_state);

        let continue_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_add(continue_epsilons, right_first_state);

        self.epsilons.list_items_set(left_last_epsilons, 0, continue_state);
        self.epsilons.list_items_set(right_last_epsilons, 0, end_state);

        self.transitions.graph_add(start_state, (0, 0), start_epsilons, 0x00);
        self.transitions.graph_add(continue_state, (0, 0), continue_epsilons, 0x00);

        let end_epsilons = self.epsilons.list_push_head();
        self.transitions.graph_add(end_state, (0, 0), end_epsilons, 0x00);
        self.epsilons.list_items_resize(end_epsilons, 1);

        self.collapsed.stack_push_front(start_state);
        self.collapsed.stack_push_front(end_epsilons);

        if self.accepting.stack_peek_front() == b'?' as u16 {
            self.accepting.stack_pop_front();
            self.accepting.stack_push_front(b'|' as u16);
        } else {
            self.accepting.stack_push_front(b'&' as u16);
        }

        idx + 1
    }

    fn handle_repetition(&mut self, idx: u16) -> u16 {
        let op_last_epsilons = self.collapsed.stack_pop_front();
        let op_first_state = self.collapsed.stack_pop_front();

        let start_state = self.next();
        let repeat_state = self.next();
        let end_state = self.next();

        let start_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_add(start_epsilons, op_first_state);

        let repeat_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_add(repeat_epsilons, start_state);
        self.epsilons.list_items_add(repeat_epsilons, end_state);

        self.epsilons.list_items_set(op_last_epsilons, 0, repeat_state);
        self.transitions.graph_add(start_state, (0, 0), start_epsilons, 0x00);
        self.transitions.graph_add(repeat_state, (0, 0), repeat_epsilons, 0x00);

        let end_epsilons = self.epsilons.list_push_head();
        self.transitions.graph_add(end_state, (0, 0), end_epsilons, 0x00);
        self.epsilons.list_items_resize(end_epsilons, 1);

        self.collapsed.stack_push_front(start_state);
        self.collapsed.stack_push_front(end_epsilons);

        idx + 1
    }

    fn handle_optionality(&mut self, idx: u16) -> u16 {
        let op_last_epsilons = self.collapsed.stack_pop_front();
        let op_first_state = self.collapsed.stack_pop_front();

        let start_state = self.next();
        let end_state = self.next();

        let start_epsilons = self.epsilons.list_push_head();
        self.epsilons.list_items_add(start_epsilons, op_first_state);
        self.epsilons.list_items_add(start_epsilons, end_state);

        self.epsilons.list_items_set(op_last_epsilons, 0, end_state);
        self.transitions.graph_add(start_state, (0, 0), start_epsilons, 0x00);

        let end_epsilons = self.epsilons.list_push_head();
        self.transitions.graph_add(end_state, (0, 0), end_epsilons, 0x00);
        self.epsilons.list_items_resize(end_epsilons, 1);

        self.collapsed.stack_push_front(start_state);
        self.collapsed.stack_push_front(end_epsilons);
        self.accepting.stack_push_front(b'?' as u16);

        idx + 1
    }

    fn handle_acceptance<const SIZE: usize>(&mut self, rpn: &RPN<SIZE>, idx: u16) -> u16 {
        self.accepting.stack_push_front(rpn.at(idx + 1).into());
        self.accepting.stack_push_front(b'#' as u16);

        idx + 2
    }

    fn build_transitions<const SIZE: usize>(&mut self, rpn: &RPN<SIZE>) {
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
                b'l' => idx = self.handle_literal(&rpn, idx),
                b'-' => idx = self.handle_positive_class(&rpn, idx),
                b'#' => idx = self.handle_acceptance(&rpn, idx),
                b'|' => idx = self.handle_alternation(idx),
                b'&' => idx = self.handle_concatenation(idx),
                b'+' => idx = self.handle_repetition(idx),
                b'?' => idx = self.handle_optionality(idx),
                _ => idx = idx + 1,
            }
        }

        let last_epsilons = self.collapsed.stack_pop_front();
        let first_state = self.collapsed.stack_pop_front();

        self.epsilons.list_items_set(start_epsilons, 0, first_state);
        self.epsilons.list_items_set(last_epsilons, 0, end_state);
    }

    fn propagate_accepting_states(&mut self) {
        self.accepting.stack_push_back(1);
        while self.accepting.stack_size_front() > 0 {
            match self.accepting.stack_pop_front() as u8 {
                b'|' => {
                    let val = self.accepting.stack_pop_back();

                    self.accepting.stack_push_back(val);
                    self.accepting.stack_push_back(val);
                }
                b'&' => {
                    let val = self.accepting.stack_pop_back();

                    self.accepting.stack_push_back(0);
                    self.accepting.stack_push_back(val);
                }
                b'#' => {
                    let val = self.accepting.stack_pop_front();

                    self.accepting.stack_pop_back();
                    self.accepting.stack_push_back(val);
                }
                b'?' => {}
                _ => {
                    let idx = self.accepting.stack_pop_front();
                    let val = self.accepting.stack_pop_back();

                    if val > 0 {
                        self.transitions.graph_set_meta(idx, val);
                    }
                }
            }
        }
    }

    fn build<const SIZE: usize>(mut self, rpn: RPN<SIZE>) -> Option<NFA> {
        self.build_transitions(&rpn);
        self.propagate_accepting_states();

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
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 73 - 73 | 0200 | 0004
        // 0004 | 74 - 74 | 0200 | 0005
        // 0005 | 61 - 61 | 0200 | 0006
        // 0006 | 72 - 72 | 0200 | 0007
        // 0007 | 74 - 74 | 0001 | 0008
        // 0008 | 00 - 00 | 0000 | 000e -> 0001

        assert_eq!(nfa.transition_count(), 9);
        assert_eq!(nfa.epsilons.list_count(), 4);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x0200)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x0200)));
        assert_eq!(nfa.transition_find(5, b'a'), Some((6, 0x0200)));
        assert_eq!(nfa.transition_find(6, b'r'), Some((7, 0x0200)));
        assert_eq!(nfa.transition_find(7, b't'), Some((8, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_class_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"-09");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 0002
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 30 - 39 | 0001 | 0004
        // 0004 | 00 - 00 | 0000 | 000e -> 0001

        assert_eq!(nfa.transition_count(), 5);
        assert_eq!(nfa.epsilons.list_count(), 4);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        for idx in 48..58 {
            assert_eq!(nfa.transition_find(3, idx), Some((4, 0x01)));
        }

        // points at epsilon
        assert_eq!(nfa.transition_find(4, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_class_repeated_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"-09+");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 0005
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 30 - 39 | 0001 | 0004
        // 0004 | 00 - 00 | 0000 | 000e -> 0006
        // 0005 | 00 - 00 | 0000 | 0013 -> 0002
        // 0006 | 00 - 00 | 0000 | 0018 -> 0005 0007
        // 0007 | 00 - 00 | 0000 | 001e -> 0001

        assert_eq!(nfa.transition_count(), 8);
        assert_eq!(nfa.epsilons.list_count(), 7);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 5);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        for idx in 48..58 {
            assert_eq!(nfa.transition_find(3, idx), Some((4, 0x01)));
        }

        // points at epsilon
        assert_eq!(nfa.transition_find(4, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 6);

        // points at epsilon
        assert_eq!(nfa.transition_find(5, 0), Some((0x13, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x13), 1);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(6, 0), Some((0x18, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x18), 2);
        assert_eq!(nfa.epsilon_items_get(0x18, 0), 5);
        assert_eq!(nfa.epsilon_items_get(0x18, 1), 7);

        // points at epsilon
        assert_eq!(nfa.transition_find(7, 0), Some((0x1e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x1e), 1);
        assert_eq!(nfa.epsilon_items_get(0x1e, 0), 1);
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
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 73 - 73 | 0200 | 0004
        // 0004 | 74 - 74 | 0200 | 0005
        // 0005 | 61 - 61 | 0200 | 0006
        // 0006 | 72 - 72 | 0200 | 0007
        // 0007 | 74 - 74 | 0001 | 0008
        // 0008 | 00 - 00 | 0000 | 000e -> 0010
        // 0009 | 00 - 00 | 0200 | 0013 -> 000a
        // 000a | 73 - 73 | 0200 | 000b
        // 000b | 74 - 74 | 0200 | 000c
        // 000c | 6f - 6f | 0200 | 000d
        // 000d | 70 - 70 | 0001 | 000e
        // 000e | 00 - 00 | 0000 | 0018 -> 0010
        // 000f | 00 - 00 | 0000 | 001d -> 0002 0009
        // 0010 | 00 - 00 | 0000 | 0023 -> 0001

        assert_eq!(nfa.transition_count(), 17);
        assert_eq!(nfa.epsilons.list_count(), 8);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 15);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon, beginning of literal
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x0200)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x0200)));
        assert_eq!(nfa.transition_find(5, b'a'), Some((6, 0x0200)));
        assert_eq!(nfa.transition_find(6, b'r'), Some((7, 0x0200)));
        assert_eq!(nfa.transition_find(7, b't'), Some((8, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 16);

        // points at epsilon, beginning of literal
        assert_eq!(nfa.transition_find(9, 0), Some((0x13, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x13), 1);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 10);

        // connects 'stop' literal
        assert_eq!(nfa.transition_find(10, b's'), Some((11, 0x0200)));
        assert_eq!(nfa.transition_find(11, b't'), Some((12, 0x0200)));
        assert_eq!(nfa.transition_find(12, b'o'), Some((13, 0x0200)));
        assert_eq!(nfa.transition_find(13, b'p'), Some((14, 0x01)));

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

    #[test]
    fn handles_converting_rpn_concat_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l5startl4stop&");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 000f
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 73 - 73 | 0200 | 0004
        // 0004 | 74 - 74 | 0200 | 0005
        // 0005 | 61 - 61 | 0200 | 0006
        // 0006 | 72 - 72 | 0200 | 0007
        // 0007 | 74 - 74 | 0000 | 0008
        // 0008 | 00 - 00 | 0000 | 000e -> 0010
        // 0009 | 00 - 00 | 0200 | 0013 -> 000a
        // 000a | 73 - 73 | 0200 | 000b
        // 000b | 74 - 74 | 0200 | 000c
        // 000c | 6f - 6f | 0200 | 000d
        // 000d | 70 - 70 | 0001 | 000e
        // 000e | 00 - 00 | 0000 | 0018 -> 0011
        // 000f | 00 - 00 | 0000 | 001d -> 0002
        // 0010 | 00 - 00 | 0000 | 0022 -> 0009
        // 0011 | 00 - 00 | 0000 | 0027 -> 0001

        assert_eq!(nfa.transition_count(), 18);
        assert_eq!(nfa.epsilons.list_count(), 9);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 15);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon, beginning of literal
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x0200)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x0200)));
        assert_eq!(nfa.transition_find(5, b'a'), Some((6, 0x0200)));
        assert_eq!(nfa.transition_find(6, b'r'), Some((7, 0x0200)));
        assert_eq!(nfa.transition_find(7, b't'), Some((8, 0x00)));

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 16);

        // points at epsilon, beginning of literal
        assert_eq!(nfa.transition_find(9, 0), Some((0x13, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x13), 1);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 10);

        // connects 'stop' literal
        assert_eq!(nfa.transition_find(10, b's'), Some((11, 0x0200)));
        assert_eq!(nfa.transition_find(11, b't'), Some((12, 0x0200)));
        assert_eq!(nfa.transition_find(12, b'o'), Some((13, 0x0200)));
        assert_eq!(nfa.transition_find(13, b'p'), Some((14, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(14, 0), Some((0x18, 0)));
        assert_eq!(nfa.epsilon_items_count(0x18), 1);
        assert_eq!(nfa.epsilon_items_get(0x18, 0), 17);

        // points at epsilon
        assert_eq!(nfa.transition_find(15, 0), Some((0x1d, 0)));
        assert_eq!(nfa.epsilon_items_count(0x1d), 1);
        assert_eq!(nfa.epsilon_items_get(0x1d, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(16, 0), Some((0x22, 0)));
        assert_eq!(nfa.epsilon_items_count(0x22), 1);
        assert_eq!(nfa.epsilon_items_get(0x22, 0), 9);

        // points at epsilon
        assert_eq!(nfa.transition_find(17, 0), Some((0x27, 0)));
        assert_eq!(nfa.epsilon_items_count(0x27), 1);
        assert_eq!(nfa.epsilon_items_get(0x27, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_optional_left_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l2st?l2op&");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 000c
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 73 - 73 | 0200 | 0004
        // 0004 | 74 - 74 | 0000 | 0005
        // 0005 | 00 - 00 | 0000 | 000e -> 0007
        // 0006 | 00 - 00 | 0000 | 0013 -> 0002 0007
        // 0007 | 00 - 00 | 0000 | 0019 -> 000d
        // 0008 | 00 - 00 | 0200 | 001e -> 0009
        // 0009 | 6f - 6f | 0200 | 000a
        // 000a | 70 - 70 | 0001 | 000b
        // 000b | 00 - 00 | 0000 | 0023 -> 000e
        // 000c | 00 - 00 | 0000 | 0028 -> 0006
        // 000d | 00 - 00 | 0000 | 002d -> 0008
        // 000e | 00 - 00 | 0000 | 0032 -> 0001

        assert_eq!(nfa.transition_count(), 15);
        assert_eq!(nfa.epsilons.list_count(), 11);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 12);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'st' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x0200)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x00)));

        // points at epsilon
        assert_eq!(nfa.transition_find(5, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 7);

        // points at epsilon
        assert_eq!(nfa.transition_find(6, 0), Some((0x13, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x13), 2);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 2);
        assert_eq!(nfa.epsilon_items_get(0x13, 1), 7);

        // points at epsilon
        assert_eq!(nfa.transition_find(7, 0), Some((0x19, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x19), 1);
        assert_eq!(nfa.epsilon_items_get(0x19, 0), 13);

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x1e, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x1e), 1);
        assert_eq!(nfa.epsilon_items_get(0x1e, 0), 9);

        // connects 'op' literal
        assert_eq!(nfa.transition_find(9, b'o'), Some((10, 0x0200)));
        assert_eq!(nfa.transition_find(10, b'p'), Some((11, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(11, 0), Some((0x23, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x23), 1);
        assert_eq!(nfa.epsilon_items_get(0x23, 0), 14);

        // points at epsilon
        assert_eq!(nfa.transition_find(12, 0), Some((0x28, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x28), 1);
        assert_eq!(nfa.epsilon_items_get(0x28, 0), 6);

        // points at epsilon
        assert_eq!(nfa.transition_find(13, 0), Some((0x2d, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x2d), 1);
        assert_eq!(nfa.epsilon_items_get(0x2d, 0), 8);

        // points at epsilon
        assert_eq!(nfa.transition_find(14, 0), Some((0x32, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x32), 1);
        assert_eq!(nfa.epsilon_items_get(0x32, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_optional_right_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l2stl2op?&");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 000c
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 73 - 73 | 0200 | 0004
        // 0004 | 74 - 74 | 0001 | 0005
        // 0005 | 00 - 00 | 0000 | 000e -> 000d
        // 0006 | 00 - 00 | 0200 | 0013 -> 0007
        // 0007 | 6f - 6f | 0200 | 0008
        // 0008 | 70 - 70 | 0001 | 0009
        // 0009 | 00 - 00 | 0000 | 0018 -> 000b
        // 000a | 00 - 00 | 0000 | 001d -> 0006 000b
        // 000b | 00 - 00 | 0000 | 0023 -> 000e
        // 000c | 00 - 00 | 0000 | 0028 -> 0002
        // 000d | 00 - 00 | 0000 | 002d -> 000a
        // 000e | 00 - 00 | 0000 | 0032 -> 0001

        assert_eq!(nfa.transition_count(), 15);
        assert_eq!(nfa.epsilons.list_count(), 11);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 12);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'st' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x0200)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(5, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 13);

        // points at epsilon
        assert_eq!(nfa.transition_find(6, 0), Some((0x13, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x13), 1);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 7);

        // connects 'op' literal
        assert_eq!(nfa.transition_find(7, b'o'), Some((8, 0x0200)));
        assert_eq!(nfa.transition_find(8, b'p'), Some((9, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(9, 0), Some((0x18, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x18), 1);
        assert_eq!(nfa.epsilon_items_get(0x18, 0), 11);

        // points at epsilon
        assert_eq!(nfa.transition_find(10, 0), Some((0x1d, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x1d), 2);
        assert_eq!(nfa.epsilon_items_get(0x1d, 0), 6);
        assert_eq!(nfa.epsilon_items_get(0x1d, 1), 11);

        // points at epsilon
        assert_eq!(nfa.transition_find(11, 0), Some((0x23, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x23), 1);
        assert_eq!(nfa.epsilon_items_get(0x23, 0), 14);

        // points at epsilon
        assert_eq!(nfa.transition_find(12, 0), Some((0x28, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x28), 1);
        assert_eq!(nfa.epsilon_items_get(0x28, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(13, 0), Some((0x2d, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x2d), 1);
        assert_eq!(nfa.epsilon_items_get(0x2d, 0), 10);

        // points at epsilon
        assert_eq!(nfa.transition_find(14, 0), Some((0x32, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x32), 1);
        assert_eq!(nfa.epsilon_items_get(0x32, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_repeat_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l4stop+");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 0008
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 73 - 73 | 0200 | 0004
        // 0004 | 74 - 74 | 0200 | 0005
        // 0005 | 6f - 6f | 0200 | 0006
        // 0006 | 70 - 70 | 0001 | 0007
        // 0007 | 00 - 00 | 0000 | 000e -> 0009
        // 0008 | 00 - 00 | 0000 | 0013 -> 0002
        // 0009 | 00 - 00 | 0000 | 0018 -> 0008 000a
        // 000a | 00 - 00 | 0000 | 001e -> 0001

        assert_eq!(nfa.transition_count(), 11);
        assert_eq!(nfa.epsilons.list_count(), 7);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 8);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 'stop' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x0200)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x0200)));
        assert_eq!(nfa.transition_find(5, b'o'), Some((6, 0x0200)));
        assert_eq!(nfa.transition_find(6, b'p'), Some((7, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(7, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 9);

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x13, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x13), 1);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(9, 0), Some((0x18, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x18), 2);
        assert_eq!(nfa.epsilon_items_get(0x18, 0), 8);
        assert_eq!(nfa.epsilon_items_get(0x18, 1), 10);

        // points at epsilon
        assert_eq!(nfa.transition_find(10, 0), Some((0x1e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x1e), 1);
        assert_eq!(nfa.epsilon_items_get(0x1e, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_zero_followed_by_two_optional_numbers_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l10l11?&l12?&");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 0012
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 30 - 30 | 0001 | 0004
        // 0004 | 00 - 00 | 0000 | 000e -> 000b
        // 0005 | 00 - 00 | 0200 | 0013 -> 0006
        // 0006 | 31 - 31 | 0001 | 0007
        // 0007 | 00 - 00 | 0000 | 0018 -> 0009
        // 0008 | 00 - 00 | 0000 | 001d -> 0005 0009
        // 0009 | 00 - 00 | 0000 | 0023 -> 000c
        // 000a | 00 - 00 | 0000 | 0028 -> 0002
        // 000b | 00 - 00 | 0000 | 002d -> 0008
        // 000c | 00 - 00 | 0000 | 0032 -> 0013
        // 000d | 00 - 00 | 0200 | 0037 -> 000e
        // 000e | 32 - 32 | 0001 | 000f
        // 000f | 00 - 00 | 0000 | 003c -> 0011
        // 0010 | 00 - 00 | 0000 | 0041 -> 000d 0011
        // 0011 | 00 - 00 | 0000 | 0047 -> 0014
        // 0012 | 00 - 00 | 0000 | 004c -> 000a
        // 0013 | 00 - 00 | 0000 | 0051 -> 0010
        // 0014 | 00 - 00 | 0000 | 0056 -> 0001

        assert_eq!(nfa.transition_count(), 21);
        assert_eq!(nfa.epsilons.list_count(), 18);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 18);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects '0' literal
        assert_eq!(nfa.transition_find(3, b'0'), Some((4, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(4, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 11);

        // points at epsilon
        assert_eq!(nfa.transition_find(5, 0), Some((0x13, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x13), 1);
        assert_eq!(nfa.epsilon_items_get(0x13, 0), 6);

        // connects '1' literal
        assert_eq!(nfa.transition_find(6, b'1'), Some((7, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(7, 0), Some((0x18, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x18), 1);
        assert_eq!(nfa.epsilon_items_get(0x18, 0), 9);

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((0x1d, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x1d), 2);
        assert_eq!(nfa.epsilon_items_get(0x1d, 0), 5);
        assert_eq!(nfa.epsilon_items_get(0x1d, 1), 9);

        // points at epsilon
        assert_eq!(nfa.transition_find(9, 0), Some((0x23, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x23), 1);
        assert_eq!(nfa.epsilon_items_get(0x23, 0), 12);

        // points at epsilon
        assert_eq!(nfa.transition_find(10, 0), Some((0x28, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x28), 1);
        assert_eq!(nfa.epsilon_items_get(0x28, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(11, 0), Some((0x2d, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x2d), 1);
        assert_eq!(nfa.epsilon_items_get(0x2d, 0), 8);

        // points at epsilon
        assert_eq!(nfa.transition_find(12, 0), Some((0x32, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x32), 1);
        assert_eq!(nfa.epsilon_items_get(0x32, 0), 19);

        // points at epsilon
        assert_eq!(nfa.transition_find(13, 0), Some((0x37, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x37), 1);
        assert_eq!(nfa.epsilon_items_get(0x37, 0), 14);

        // connects '2' literal
        assert_eq!(nfa.transition_find(14, b'2'), Some((15, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(15, 0), Some((0x3c, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x3c), 1);
        assert_eq!(nfa.epsilon_items_get(0x3c, 0), 17);

        // points at epsilon
        assert_eq!(nfa.transition_find(16, 0), Some((0x41, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x41), 2);
        assert_eq!(nfa.epsilon_items_get(0x41, 0), 13);
        assert_eq!(nfa.epsilon_items_get(0x41, 1), 17);

        // points at epsilon
        assert_eq!(nfa.transition_find(17, 0), Some((0x47, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x47), 1);
        assert_eq!(nfa.epsilon_items_get(0x47, 0), 20);

        // points at epsilon
        assert_eq!(nfa.transition_find(18, 0), Some((0x4c, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x4c), 1);
        assert_eq!(nfa.epsilon_items_get(0x4c, 0), 10);

        // points at epsilon
        assert_eq!(nfa.transition_find(19, 0), Some((0x51, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x51), 1);
        assert_eq!(nfa.epsilon_items_get(0x51, 0), 16);

        // points at epsilon
        assert_eq!(nfa.transition_find(20, 0), Some((0x56, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x56), 1);
        assert_eq!(nfa.epsilon_items_get(0x56, 0), 1);
    }

    #[test]
    fn handles_converting_rpn_accept_to_nfa() {
        let builder = Builder::new();
        let rpn: RPN<4096> = RPN::from_bytes(b"l1s#x");

        let nfa = match builder.build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        // 0000 | 00 - 00 | 0000 | 0000 -> 0002
        // 0001 | 00 - 00 | 0000 | 0005 ->
        // 0002 | 00 - 00 | 0200 | 0009 -> 0003
        // 0003 | 73 - 73 | 0078 | 0004
        // 0004 | 00 - 00 | 0000 | 000e -> 0001

        assert_eq!(nfa.transition_count(), 5);
        assert_eq!(nfa.epsilons.list_count(), 4);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((0x00, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x00), 1);
        assert_eq!(nfa.epsilon_items_get(0x00, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(1, 0), Some((0x05, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x05), 0);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0x09, 0x0200)));
        assert_eq!(nfa.epsilon_items_count(0x09), 1);
        assert_eq!(nfa.epsilon_items_get(0x09, 0), 3);

        // connects 's' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x78)));

        // points at epsilon
        assert_eq!(nfa.transition_find(4, 0), Some((0x0e, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0x0e), 1);
        assert_eq!(nfa.epsilon_items_get(0x0e, 0), 1);
    }
}
