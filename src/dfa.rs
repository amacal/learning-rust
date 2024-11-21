use super::graph::*;
use super::heap::*;
use super::list::*;
use super::nfa::*;

use std::ops::Shr;

pub struct DFA {
    transitions: Graph<4096, GuardSegfault>,
}

impl DFA {
    fn from(transitions: Graph<4096, GuardSegfault>) -> Self {
        Self {
            transitions: transitions,
        }
    }

    pub fn build(nfa: NFA) -> Option<Self> {
        Builder::new().build(nfa)
    }

    pub fn transition_count(&self) -> u16 {
        self.transitions.graph_count()
    }

    pub fn transition_find(&self, src: u16, via: u8) -> Option<(u16, u16)> {
        self.transitions.graph_find(src, via)
    }

    pub fn print(&self) {
        for idx in 0..self.transition_count() {
            let transition = self.transitions.graph_at(idx);
            println!("{:04x} | {:02x} - {:02x} | {:04x} | {:04x}", transition.0, transition.1 .0, transition.1 .1, transition.2, transition.3);
        }

        println!();
    }

    pub fn traverse(&self, data: &[u8], start: u16) -> Option<(u16, usize)> {
        let mut current = (start, 0);
        let mut best = None;

        for (idx, &val) in data.iter().enumerate() {
            current = match self.transition_find(current.0, val) {
                None => break,
                Some((state, meta)) => {
                    if meta & 0xff > 0x00 {
                        best = Some((meta & 0xff, idx));
                    }

                    (state, idx)
                }
            };
        }

        best
    }
}

struct Builder {
    counter: u16,
    transitions: Graph<4096, GuardSegfault>,
    worklist: Collection<4096, GuardWrapping>,
    closures: Collection<4096, GuardSegfault>,
    intervals: Collection<4096, GuardWrapping>,
}

impl Builder {
    fn new() -> Self {
        Self {
            counter: 0,
            transitions: Graph::new(),
            worklist: Collection::new(),
            closures: Collection::new(),
            intervals: Collection::new(),
        }
    }

    fn next(&mut self) -> u16 {
        self.counter = self.counter.wrapping_add(1);
        self.counter.wrapping_sub(1)
    }

    fn revert(&mut self) {
        self.counter = self.counter.wrapping_sub(1);
    }

    fn build(mut self, nfa: NFA) -> Option<DFA> {
        self.nfa_to_dfa(nfa);
        Some(DFA::from(self.transitions))
    }

    fn nfa_close_epsilon(&mut self, nfa: &NFA, worklist: u16) -> (u16, bool) {
        let mut changed = true;
        let mut epsilon = false;

        let mut size;
        let closure = self.closures.list_push_head();

        for off in 1..self.worklist.list_items_count(worklist) {
            let val = self.worklist.list_items_get(worklist, off);
            self.closures.list_items_add(closure, val & 0x7fff);
            epsilon = epsilon | (val & 0x8000 == 0x8000);
        }

        // a state where the worklist will point if successfully closed
        let next = self.worklist.list_items_get(worklist, 0);
        self.worklist.list_items_set(worklist, 0, 0);

        println!();
        print!("closing {closure:04x} -> ");

        for off in 0..self.closures.list_items_count(closure) {
            print!("{:04x} ", self.closures.list_items_get(closure, off));
        }

        println!();

        while changed {
            changed = false;
            size = self.closures.list_items_count(closure);

            for off in 0..self.closures.list_items_count(closure) {
                let src = self.closures.list_items_get(closure, off);
                if let Some((epsilon_idx, meta)) = nfa.transition_find(src, 0) {
                    for off in 0..nfa.epsilon_items_count(epsilon_idx) {
                        let val = nfa.epsilon_items_get(epsilon_idx, off);

                        if meta & 0x0200 == 0x0200 {
                            self.worklist.list_items_add(worklist, val);
                        } else {
                            if !self.closures.list_items_contains(closure, val, size) {
                                self.closures.list_items_add(closure, val);
                                changed = true;
                                epsilon = true;
                            }
                        }
                    }
                }
            }

            if changed {
                self.closures.list_items_sort(closure);
                self.closures.list_items_distinct(closure);
            }

            print!("closing {closure:04x} -> ");

            for off in 0..self.closures.list_items_count(closure) {
                print!("{:04x} ", self.closures.list_items_get(closure, off));
            }

            println!();
        }

        self.worklist.list_items_sort(worklist);
        self.worklist.list_items_distinct(worklist);

        print!("closed  {worklist:04x} -> ");

        for off in 0..self.worklist.list_items_count(worklist) {
            print!("{:04x} ", self.worklist.list_items_get(worklist, off));
        }

        println!("\n");

        let count = self.worklist.list_items_count(worklist);
        self.closures.list_items_resize(closure, 0);

        for off in 0..count {
            let val = self.worklist.list_items_get(worklist, off);
            if val & 0x8000 == 0x0000 {
                self.closures.list_items_add(closure, val);
            }
        }

        self.closures.list_items_hash(closure);
        self.closures.list_items_add(closure, next);

        self.worklist.list_items_resize(worklist, 0);
        self.worklist.list_items_add(worklist, next);

        (closure, epsilon)
    }

    fn nfa_record_state(&mut self, nfa: &NFA, states: u16, worklist: u16) -> u16 {
        // find all transition via epsilon and find potentially available idx
        let (closure, epsilon) = self.nfa_close_epsilon(nfa, worklist);
        let length = self.closures.list_items_count(closure) - 1;
        let idx = self.closures.set_items_find(states, closure, length);

        if idx > 0 {
            // closure is useless and working list needs to be consumed from head
            self.closures.list_pop_head();
            self.worklist.list_pop_head();

            println!("temporary closure {closure:04x} already exists, reusing {idx:04x}");
            return self.closures.list_items_get(idx, length);
        }

        // there is no reason to cache closures not going through any epsilon
        if epsilon == false || length == 0 {
            self.closures.list_pop_head();
        } else {
            self.closures.set_items_add(states, closure);
        }

        // entire closure is copied to a working list
        for off in 0..self.closures.list_items_count(closure) - 1 {
            let val = self.closures.list_items_get(closure, off);
            self.worklist.list_items_add(worklist, val);
        }

        println!("temporary closure {closure:04x} persisted in the lookup");
        self.closures.list_items_get(closure, length)
    }

    fn nfa_iteration(&mut self, current: u16, nfa: &NFA, via: (u8, u8)) {
        let dst = self.next();
        let mut accepting = 0x00;
        let worklist = self.worklist.list_push_head();

        let size = self.worklist.list_items_count(current);
        let src = self.worklist.list_items_get(current, 0);

        println!(
            "iteration nfa-src={src:04x}, dfa-dst={dst:04x}, worklist-out={worklist:04x}, worklist-in-size={size:04x}, via=({:02x}, {:02x})",
            via.0, via.1
        );

        // add NFA's dst state
        self.worklist.list_items_add(worklist, dst);

        // let's try to add a valid transition
        for idx in 1..size {
            let src = self.worklist.list_items_get(current, idx);
            if let Some((dst, meta)) = nfa.transition_find(src, via.0) {
                let dst = if meta & 0x0200 == 0x0200 { dst } else { dst | 0x8000 };
                self.worklist.list_items_add(worklist, dst);
                accepting = accepting | (meta & 0xff);
            }
        }

        // if working list contains any state
        if self.worklist.list_items_count(worklist) > 1 {
            let target = self.nfa_record_state(nfa, 0, worklist);
            self.transitions.graph_add(src, (via.0, via.1), target, accepting);
            println!("appending dfa transition {src:04x} | {:02x} - {:02x} | {target:04x} | {accepting:04x}", via.0, via.1);

            if target != dst {
                self.revert();
            }
        } else {
            self.revert();
            self.worklist.list_pop_head();
        }
    }

    fn merge_intervals(&mut self, worklist: u16, nfa: &NFA) -> u16 {
        let ranges = self.intervals.list_push_head();

        for off in 1..self.worklist.list_items_count(worklist) {
            let idx = self.worklist.list_items_get(worklist, off);
            let range = nfa.transition_find_all(idx);

            if let Some(range) = range {
                for idx in range.0..=range.1 {
                    let val = nfa.transition_at(idx);
                    let (from, to) = (val.1 .0 as u16, val.1 .1 as u16);

                    if from > 0 {
                        self.intervals.list_items_add(ranges, from.rotate_left(8) | to);
                    }
                }
            }
        }

        self.intervals.list_items_sort(ranges);
        self.intervals.list_items_distinct(ranges);

        let deltas = self.intervals.list_push_head();

        for off in 0..self.intervals.list_items_count(ranges) {
            let range = self.intervals.list_items_get(ranges, off);

            self.intervals.list_items_add(deltas, range & 0xff00);
            self.intervals.list_items_add(deltas, (range & 0x00ff).rotate_left(8) | 0x01);
        }

        self.intervals.list_items_sort(deltas);

        let intervals = self.intervals.list_push_head();
        let count = self.intervals.list_items_count(deltas);
        let mut depth = 0u16;
        let mut last: Option<u16> = None;

        for off in 0..count {
            let val = self.intervals.list_items_get(deltas, off);

            if let Some(x) = last {
                if depth > 0 {
                    let end: u16 = val.shr(8);
                    let end = end.wrapping_sub(if val & 0x01 == 0x01 { 0 } else { 1 });

                    if x.shr(8) <= end {
                        self.intervals.list_items_add(intervals, x | end);
                    }

                    last = Some(end.wrapping_add(1).rotate_left(8));
                }
            }

            if val & 0x01 == 0x00 {
                depth = depth.wrapping_add(1);
            } else {
                depth = depth.wrapping_sub(1);
            }

            if let Some(x) = last {
                if x < val & 0xff00 {
                    last = Some(val & 0xff00);
                }
            } else {
                last = Some(val & 0xff00);
            }
        }

        println!("intervals, used={}", self.intervals.usage());
        self.intervals.print();

        self.intervals.list_pop_tail();
        self.intervals.list_pop_tail();

        intervals
    }

    fn nfa_to_dfa(&mut self, nfa: NFA) {
        let starting = self.next();
        let states = self.closures.set_push_head(8);
        let worklist = self.worklist.list_push_head();

        // add NFA's starting state followed by DFA's starting state
        self.worklist.list_items_add(worklist, starting);
        self.worklist.list_items_add(worklist, 0);

        println!("worklist, used={}", self.worklist.usage());
        self.worklist.print();

        // the first round of epsilon discovery
        self.nfa_record_state(&nfa, states, worklist);

        println!("closures, used={}", self.closures.usage());
        self.closures.print();

        while self.worklist.list_count() > 0 {
            println!("worklist, cnt={}, used={}", self.worklist.list_count(), self.worklist.usage());
            self.worklist.print();

            // pop a list from the worklist, each worklist contains at 0 the source state id
            // the remaining items are reachable from the state id via epsilon
            let current = self.worklist.list_pop_tail();
            let intervals = self.merge_intervals(current, &nfa);

            println!("handling worklist={current:04x}, intervals={intervals:04x} ...");

            for off in 0..self.intervals.list_items_count(intervals) {
                let encoded = self.intervals.list_items_get(intervals, off);
                let (from, to) = (encoded.shr(8) as u8, (encoded & 0xff) as u8);

                self.nfa_iteration(current, &nfa, (from, to));
            }

            self.intervals.list_pop_tail();

            println!("\nclosures, used={}", self.closures.usage());
            self.closures.print();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpn::*;
    use std::i16;

    #[test]
    fn handles_closing_nfa_from_epsilon_state() {
        let regex = b"start|stop\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let mut builder = Builder::new();
        let worklist = builder.worklist.list_push_head();

        // 99 is a state followed by starting state as an epsilon
        builder.worklist.list_items_add(worklist, 99);
        builder.worklist.list_items_add(worklist, 0);

        let (closure, epsilon) = builder.nfa_close_epsilon(&nfa, worklist);

        assert_eq!(epsilon, true);
        assert_eq!(builder.closures.list_items_count(closure), 3);
        assert_eq!(builder.closures.list_items_get(closure, 0), 3);
        assert_eq!(builder.closures.list_items_get(closure, 1), 10);

        // artificially inserted after hashing
        assert_eq!(builder.closures.list_items_get(closure, 2), 99);
    }

    #[test]
    fn handles_closing_nfa_from_non_epsilon_state() {
        let regex = b"start|stop\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let mut builder = Builder::new();
        let worklist = builder.worklist.list_push_head();

        // 99 is a state followed by a non-epsilon 1 state
        builder.worklist.list_items_add(worklist, 99);
        builder.worklist.list_items_add(worklist, 1);

        let (closure, epsilon) = builder.nfa_close_epsilon(&nfa, worklist);

        assert_eq!(epsilon, false);
        assert_eq!(builder.closures.list_items_count(closure), 2);
        assert_eq!(builder.closures.list_items_get(closure, 0), 1);

        // artificially inserted after hashing
        assert_eq!(builder.closures.list_items_get(closure, 1), 99);
    }

    #[test]
    fn handles_recording_nfa_state_not_repeated() {
        let regex = b"start|stop\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let mut builder = Builder::new();
        let worklist = builder.worklist.list_push_head();
        let states = builder.closures.set_push_head(8);

        builder.worklist.list_items_add(worklist, 0);
        builder.nfa_record_state(&nfa, states, worklist);

        // working list is not consumed, but reused
        assert_eq!(builder.worklist.list_count(), 1);
    }

    #[test]
    fn handles_recording_nfa_state_repeated() {
        let regex = b"start|stop\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
            Some(nfa) => nfa,
            None => return assert!(false),
        };

        let mut builder = Builder::new();
        let states = builder.closures.set_push_head(8);

        let worklist = builder.worklist.list_push_head();
        builder.worklist.list_items_add(worklist, 13);
        builder.worklist.list_items_add(worklist, 0);
        builder.nfa_record_state(&nfa, states, worklist);

        let worklist = builder.worklist.list_push_head();
        builder.worklist.list_items_add(worklist, 13);
        builder.worklist.list_items_add(worklist, 0);
        builder.nfa_record_state(&nfa, states, worklist);

        // second working list is consumed
        assert_eq!(builder.worklist.list_count(), 1);
    }

    #[test]
    fn handles_converting_nfa_to_dfa_either() {
        let regex = b"start#x|stop#y\0".as_ptr();

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

        assert_eq!(dfa.transition_count(), 7);

        assert_eq!(dfa.transition_find(0, b's'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(1, b't'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(2, b'a'), Some((3, 0x00)));
        assert_eq!(dfa.transition_find(2, b'o'), Some((4, 0x00)));
        assert_eq!(dfa.transition_find(3, b'r'), Some((5, 0x00)));
        assert_eq!(dfa.transition_find(4, b'p'), Some((6, 0x79)));
        assert_eq!(dfa.transition_find(5, b't'), Some((7, 0x78)));
    }

    #[test]
    fn handles_converting_nfa_to_dfa_repeat() {
        let regex = b"(stop)+#x\0".as_ptr();

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

        assert_eq!(dfa.transition_count(), 4);

        assert_eq!(dfa.transition_find(0, b's'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(1, b't'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(2, b'o'), Some((3, 0x00)));
        assert_eq!(dfa.transition_find(3, b'p'), Some((0, 0x78)));
    }

    #[test]
    fn handles_converting_nfa_to_dfa_optional() {
        let regex = b"(st)?op\0".as_ptr();

        let rpn: RPN<4096> = match RPN::build(regex) {
            Some(rpn) => rpn,
            None => return assert!(false),
        };

        let nfa = match NFA::build(rpn) {
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

        let dfa = match DFA::build(nfa) {
            Some(dfa) => dfa,
            None => return assert!(false),
        };

        // 0000 | 6f - 6f | 0001 | 0000
        // 0000 | 73 - 73 | 0200 | 0000
        // 0001 | 70 - 70 | 0201 | 0001
        // 0002 | 74 - 74 | 0004 | 0000
        // 0004 | 6f - 6f | 0005 | 0000
        // 0005 | 70 - 70 | 0006 | 0001

        assert_eq!(dfa.transition_count(), 6);

        assert_eq!(dfa.transition_find(0, b'o'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(0, b's'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(1, b'p'), Some((3, 0x01)));
        assert_eq!(dfa.transition_find(2, b't'), Some((4, 0x00)));
        assert_eq!(dfa.transition_find(4, b'o'), Some((5, 0x00)));
        assert_eq!(dfa.transition_find(5, b'p'), Some((6, 0x01)));
    }

    #[test]
    fn handles_converting_nfa_to_dfa_class() {
        let regex = b"[0-9]+\0".as_ptr();

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

        assert_eq!(dfa.transition_count(), 1);

        assert_eq!(dfa.transition_find(0, b'0'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'1'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'2'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'3'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'4'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'5'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'6'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'7'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'8'), Some((0, 0x01)));
        assert_eq!(dfa.transition_find(0, b'9'), Some((0, 0x01)));
    }

    #[test]
    fn handles_traversing_dfa_either() {
        let regex = b"start|stop\0".as_ptr();

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

        // fully accepted
        assert_eq!(dfa.traverse(b"start", 0), Some((1, 4)));
        assert_eq!(dfa.traverse(b"stop", 0), Some((1, 3)));

        // not accepted
        assert_eq!(dfa.traverse(b"stort", 0), None);
        assert_eq!(dfa.traverse(b"stap", 0), None);
    }

    #[test]
    fn handles_traversing_dfa_repeat() {
        let regex = b"(start|stop)+\0".as_ptr();

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

        // fully accepted
        assert_eq!(dfa.traverse(b"start", 0), Some((1, 4)));
        assert_eq!(dfa.traverse(b"stop", 0), Some((1, 3)));
        assert_eq!(dfa.traverse(b"startstop", 0), Some((1, 8)));
        assert_eq!(dfa.traverse(b"stopstart", 0), Some((1, 8)));

        // partially accepted
        assert_eq!(dfa.traverse(b"startsta", 0), Some((1, 4)));
        assert_eq!(dfa.traverse(b"stopsto", 0), Some((1, 3)));

        // not accepted
        assert_eq!(dfa.traverse(b"sto", 0), None);
        assert_eq!(dfa.traverse(b"sta", 0), None);
    }

    #[test]
    fn handles_traversing_dfa_optional_first() {
        let regex = b"(start)?stop\0".as_ptr();

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

        // fully accepted
        assert_eq!(dfa.traverse(b"stop", 0), Some((1, 3)));
        assert_eq!(dfa.traverse(b"startstop", 0), Some((1, 8)));

        // partially accepted
        assert_eq!(dfa.traverse(b"stopsto", 0), Some((1, 3)));
        assert_eq!(dfa.traverse(b"stopstart", 0), Some((1, 3)));

        // not accepted
        assert_eq!(dfa.traverse(b"start", 0), None);
        assert_eq!(dfa.traverse(b"startsta", 0), None);
    }

    #[test]
    fn handles_traversing_dfa_optional_last() {
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

        // fully accepted
        assert_eq!(dfa.traverse(b"start", 0), Some((120, 4)));
        assert_eq!(dfa.traverse(b"startstop", 0), Some((121, 8)));

        // partially accepted
        assert_eq!(dfa.traverse(b"startsto", 0), Some((120, 4)));
        assert_eq!(dfa.traverse(b"startstart", 0), Some((120, 4)));

        // not accepted
        assert_eq!(dfa.traverse(b"stop", 0), None);
    }

    #[test]
    fn handles_dividing_by_three() {
        let regex = b"(0|(1(01*(00)*0)*1)*)*\0".as_ptr();

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

        for num in 0..1000000 {
            let binary = format!("{:b}", num);
            let bytes = binary.as_bytes();

            let result = match dfa.traverse(bytes, 0) {
                Some((_, idx)) => idx,
                _ => return assert!(num % 3 != 0),
            };

            assert_eq!(result, bytes.len() - 1);
        }
    }

    #[test]
    fn handles_traversing_dfa_number_regex() {
        let regex = b"-?(0|[1-9][0-9]*)(.[0-9]+)?([eE]([+]|-)?[0-9]+)?\0".as_ptr();

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

        // fully accepted
        assert_eq!(dfa.traverse(b"0", 0), Some((1, 0)));
        assert_eq!(dfa.traverse(b"12", 0), Some((1, 1)));
        assert_eq!(dfa.traverse(b"123", 0), Some((1, 2)));
        assert_eq!(dfa.traverse(b"1e3", 0), Some((1, 2)));
        assert_eq!(dfa.traverse(b"-123", 0), Some((1, 3)));
        assert_eq!(dfa.traverse(b"12.34", 0), Some((1, 4)));
        assert_eq!(dfa.traverse(b"-0.567", 0), Some((1, 5)));
        assert_eq!(dfa.traverse(b"1.23e10", 0), Some((1, 6)));
        assert_eq!(dfa.traverse(b"-4.56E-3", 0), Some((1, 7)));

        // partially accepted
        assert_eq!(dfa.traverse(b"123.", 0), Some((1, 2)));
        assert_eq!(dfa.traverse(b"1.2e", 0), Some((1, 2)));

        // not accepted
        assert_eq!(dfa.traverse(b".567", 0), None);
        assert_eq!(dfa.traverse(b"+123", 0), None);
    }

    #[test]
    fn handles_traversing_dfa_ipv4_regex() {
        let regex = b"(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9])(.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9]))(.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9]))(.(25[0-5]|2[0-4][0-9]|1[0-9][0-9]|[1-9]?[0-9]))\0".as_ptr();

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

        // fully accepted
        assert_eq!(dfa.traverse(b"0.0.0.0", 0), Some((1, 6)));
        assert_eq!(dfa.traverse(b"1.2.3.4", 0), Some((1, 6)));
        assert_eq!(dfa.traverse(b"127.0.0.1", 0), Some((1, 8)));
        assert_eq!(dfa.traverse(b"172.16.0.0", 0), Some((1, 9)));
        assert_eq!(dfa.traverse(b"10.0.0.255", 0), Some((1, 9)));
        assert_eq!(dfa.traverse(b"192.168.1.1", 0), Some((1, 10)));
        assert_eq!(dfa.traverse(b"255.255.255.255", 0), Some((1, 14)));

        // partially accepted
        assert_eq!(dfa.traverse(b"192.168.1.256", 0), Some((1, 11)));
        assert_eq!(dfa.traverse(b"192.168.1.1.1", 0), Some((1, 10)));
        assert_eq!(dfa.traverse(b"192.168.1.1.", 0), Some((1, 10)));

        // not accepted
        assert_eq!(dfa.traverse(b"256.256.256.256", 0), None);
        assert_eq!(dfa.traverse(b"192.168.1", 0), None);
        assert_eq!(dfa.traverse(b"192.168..1", 0), None);
        assert_eq!(dfa.traverse(b"300.168.1.1", 0), None);
        assert_eq!(dfa.traverse(b"abc.def.ghi.jkl", 0), None);
        assert_eq!(dfa.traverse(b"192.168.1.-1", 0), None);
        assert_eq!(dfa.traverse(b"192.168.1,1", 0), None);
    }

    #[test]
    fn handles_traversing_dfa_i16_regex() {
        let regex = b"-32768|-?3276[0-7]|-?327[0-5][0-9]|-?32[0-6][0-9][0-9]|-?3[0-1][0-9][0-9][0-9]|-?[12][0-9][0-9][0-9][0-9]|-?[1-9][0-9][0-9][0-9]|-?[1-9][0-9][0-9]|-?[1-9][0-9]|-?[1-9]|0\0".as_ptr();

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

        let min = (i16::MIN as i32).wrapping_sub(1000);
        let max = (i16::MAX as i32).wrapping_add(1000);

        for num in min..=max {
            let binary = format!("{}", num);
            let bytes = binary.as_bytes();

            let result = match dfa.traverse(bytes, 0) {
                Some((_, idx)) => idx,
                _ => return assert!(false),
            };

            match (num < i16::MIN.into(), num > i16::MAX.into()) {
                (true, _) => assert_eq!(result, bytes.len() - 2),
                (_, true) => assert_eq!(result, bytes.len() - 2),
                _ => assert_eq!(result, bytes.len() - 1),
            }
        }
    }
}
