mod array;
mod heap;
mod lexer;
mod list;
mod rpn;
mod graph;
mod nfa;

use heap::*;
use list::*;
use graph::*;
use nfa::*;

use std::ops::Shr;

fn main() {
    // Optional sign part: "-"
    let minus_sign = Regex::Literal(b"-");
    let sign = Regex::Optional(&minus_sign); // Optional("-")

    // Integer part: "0" or [1-9]\d*
    let zero = Regex::Literal(b"0");
    let non_zero_digit = Regex::Class(b'1', b'9');
    let any_digit = Regex::Class(b'0', b'9');
    let any_digit = &Regex::Repeat(&any_digit);
    let integer_with_leading_digit = Regex::Concat(&non_zero_digit, &any_digit);
    let integer_part = Regex::Either(&zero, &integer_with_leading_digit);

    // Decimal part: (?:\.\d+)?
    let decimal_point = Regex::Literal(b".");
    let decimal_digits = Regex::Repeat(&any_digit);
    let decimal_with_point = Regex::Concat(&decimal_point, &decimal_digits);
    let decimal_part = Regex::Optional(&decimal_with_point);

    // Exponent part: (?:[eE][-+]?\d+)?
    let e_literal = Regex::Literal(b"e");
    let E_literal = Regex::Literal(b"E");
    let exponent_marker = Regex::Either(&e_literal, &E_literal);

    let plus_sign = Regex::Literal(b"+");
    let signs = &Regex::Either(&minus_sign, &plus_sign);
    let exponent_sign = Regex::Optional(&signs);

    let exponent_digits = Regex::Repeat(&any_digit);
    let exponent_with_sign = Regex::Concat(&exponent_sign, &exponent_digits);
    let exponent_part = &Regex::Concat(&exponent_marker, &exponent_with_sign);
    let exponent_part = Regex::Optional(&exponent_part);

    // Combine parts for full JSON number
    let integer_and_decimal = Regex::Concat(&integer_part, &decimal_part);
    let number_with_exponent = Regex::Concat(&integer_and_decimal, &exponent_part);
    let regex = Regex::Concat(&sign, &number_with_exponent);

    let mut workbench = Workbench::new();
    let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

    workbench.regex_to_nfa(&regex, &mut nfa);

    println!("nfa, transitions={}", nfa.transition_count());
    nfa.print();

    workbench.nfa_to_dfa(&nfa, &mut dfa);

    println!("dfa, transitions={}", dfa.transition_count());
    dfa.print();
}

enum Regex<'a> {
    Literal(&'a [u8]),
    Class(u8, u8),
    Either(&'a Regex<'a>, &'a Regex<'a>),
    Concat(&'a Regex<'a>, &'a Regex<'a>),
    Repeat(&'a Regex<'a>),
    Optional(&'a Regex<'a>),
}

struct DFA {
    counter: u16,
    transitions: Graph<4096, GuardDisabled>,
}

impl DFA {
    fn new() -> Self {
        Self {
            counter: 0,
            transitions: Graph::new(),
        }
    }

    fn next(&mut self) -> u16 {
        self.counter = self.counter.wrapping_add(1);
        self.counter.wrapping_sub(1)
    }

    fn next_revert(&mut self) {
        self.counter = self.counter.wrapping_sub(1);
    }

    fn transition_at(&self, idx: u16) -> (u16, (u8, u8), u16, u16) {
        self.transitions.graph_at(idx)
    }

    fn transition_add(&mut self, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        self.transitions.graph_add(src, via, dst, metadata);
    }

    fn transition_count(&self) -> u16 {
        self.transitions.graph_count()
    }

    fn transition_find(&self, src: u16, via: u8) -> Option<(u16, u16)> {
        self.transitions.graph_find(src, via)
    }

    fn print(&self) {
        for idx in 0..self.transition_count() {
            let transition = self.transition_at(idx);
            println!(
                "{:04x} | {:02x} - {:02x} | {:04x} | {:04x}",
                transition.0, transition.1 .0, transition.1 .1, transition.2, transition.3
            );
        }

        println!();
    }

    fn traverse(&self, data: &[u8], start: u16) -> Option<(u16, usize)> {
        let mut current = (start, 0);
        let mut best = None;

        for (idx, &val) in data.iter().enumerate() {
            current = match self.transition_find(current.0, val) {
                None => break,
                Some((state, meta)) => {
                    if meta & 0x01 == 0x01 {
                        best = Some((state, idx));
                    }

                    (state, idx)
                }
            };
        }

        best
    }
}

struct Workbench {
    worklist: Collection<4096, GuardWrapping>,
    closures: Collection<4096, GuardSegfault>,
    intervals: Collection<4096, GuardWrapping>,
}

impl Workbench {
    fn new() -> Self {
        Self {
            worklist: Collection::new(),
            closures: Collection::new(),
            intervals: Collection::new(),
        }
    }

    fn states_new(&mut self) -> u16 {
        self.closures.set_push_head(8)
    }

    fn closures_new(&mut self) -> u16 {
        self.closures.list_push_head()
    }

    fn closures_items_count(&self, idx: u16) -> u16 {
        self.closures.list_items_count(idx)
    }

    fn closures_items_get(&self, idx: u16, off: u16) -> u16 {
        self.closures.list_items_get(idx, off)
    }

    fn worklist_new(&mut self) -> u16 {
        self.worklist.list_push_head()
    }

    fn worklist_count(&self) -> u16 {
        self.worklist.list_count()
    }

    fn worklist_items_add(&mut self, idx: u16, item: u16) {
        self.worklist.list_items_add(idx, item);
    }

    fn worklist_items_count(&mut self, idx: u16) -> u16 {
        self.worklist.list_items_count(idx)
    }

    fn regex_to_nfa(&mut self, regex: &Regex, nfa: &mut NFA) -> (u16, u16) {
        fn into_nfa(node: &Regex, nfa: &mut NFA, accepting: bool) -> (u16, u16) {
            match node {
                Regex::Literal(value) => {
                    let zero = nfa.next();
                    let first = nfa.next();

                    let mut last = first;
                    let eps = nfa.epsilon_new();

                    nfa.epsilon_items_add(eps, first);
                    nfa.transition_add(zero, (0, 0), eps, 0x02);

                    for (idx, &val) in value.iter().enumerate() {
                        let next = nfa.next();
                        let inside = idx < value.len() - 1;

                        let mut meta = if inside { 0x02 } else { 0x00 };
                        meta += if accepting && !inside { 0x01 } else { 0x00 };

                        nfa.transition_add(last, (val, val), next, meta);
                        last = next;
                    }

                    (zero, last)
                }
                Regex::Class(from, to) => {
                    let zero = nfa.next();
                    let first = nfa.next();

                    let last = nfa.next();
                    let eps = nfa.epsilon_new();

                    nfa.epsilon_items_add(eps, first);
                    nfa.transition_add(zero, (0, 0), eps, 0x02);

                    let meta = if accepting { 0x01 } else { 0x00 };
                    nfa.transition_add(first, (*from, *to), last, meta);

                    (zero, last)
                }
                Regex::Either(left, right) => {
                    let first = nfa.next();

                    let ep1 = nfa.epsilon_new();
                    nfa.transition_add(first, (0, 0), ep1, 0);
                    nfa.epsilon_items_resize(ep1, 2);

                    let ep2 = nfa.epsilon_new();
                    nfa.epsilon_items_resize(ep2, 1);

                    let left = into_nfa(left, nfa, accepting);
                    nfa.epsilon_items_set(ep1, 0, left.0);
                    nfa.transition_add(left.1, (0, 0), ep2, 0);

                    let ep3 = nfa.epsilon_new();
                    nfa.epsilon_items_resize(ep3, 1);

                    let right = into_nfa(right, nfa, accepting);
                    nfa.epsilon_items_set(ep1, 1, right.0);
                    nfa.transition_add(right.1, (0, 0), ep3, 0);

                    let last = nfa.next();
                    nfa.epsilon_items_set(ep2, 0, last);
                    nfa.epsilon_items_set(ep3, 0, last);

                    (first, last)
                }
                Regex::Concat(left, right) => {
                    let first = nfa.next();
                    let ep1 = nfa.epsilon_new();

                    nfa.transition_add(first, (0, 0), ep1, 0);
                    nfa.epsilon_items_resize(ep1, 1);

                    let left = into_nfa(left, nfa, false);
                    nfa.epsilon_items_set(ep1, 0, left.0);

                    let ep2 = nfa.epsilon_new();
                    nfa.transition_add(left.1, (0, 0), ep2, 0);
                    nfa.epsilon_items_resize(ep2, 1);

                    let right = into_nfa(right, nfa, accepting);
                    nfa.epsilon_items_set(ep2, 0, right.0);

                    (first, right.1)
                }
                Regex::Repeat(target) => {
                    let first = nfa.next();
                    let ep1 = nfa.epsilon_new();

                    nfa.transition_add(first, (0, 0), ep1, 0x00);
                    nfa.epsilon_items_resize(ep1, 1);

                    let ep2 = nfa.epsilon_new();
                    nfa.epsilon_items_resize(ep2, 2);

                    let target = into_nfa(target, nfa, accepting);
                    nfa.epsilon_items_set(ep1, 0, target.0);
                    nfa.transition_add(target.1, (0, 0), ep2, 0);

                    let last = nfa.next();
                    nfa.epsilon_items_set(ep2, 0, last);
                    nfa.epsilon_items_set(ep2, 1, first);

                    (first, last)
                }
                Regex::Optional(target) => {
                    let first = nfa.next();
                    let ep1 = nfa.epsilon_new();

                    nfa.transition_add(first, (0, 0), ep1, 0x00);
                    nfa.epsilon_items_resize(ep1, 2);

                    let target = into_nfa(target, nfa, accepting);
                    let last = nfa.next();

                    nfa.epsilon_items_set(ep1, 0, target.0);
                    nfa.transition_add(target.1, (0, 0), ep1, 0);

                    nfa.epsilon_items_set(ep1, 0, target.0);
                    nfa.epsilon_items_set(ep1, 1, last);

                    (first, last)
                }
            }
        }

        let first = nfa.next();
        let last = nfa.next();

        let t1 = nfa.transition_inc();
        let refs = into_nfa(regex, nfa, true);

        let ep1 = nfa.epsilon_new();
        nfa.epsilon_items_add(ep1, refs.0);

        let ep2 = nfa.epsilon_new();
        nfa.epsilon_items_add(ep2, last);

        nfa.transition_set(t1, first, (0, 0), ep1, 0);
        nfa.transition_add(refs.1, (0, 0), ep2, 0);

        (first, last)
    }

    fn nfa_close_epsilon(&mut self, nfa: &NFA, worklist: u16) -> (u16, bool) {
        let mut changed = true;
        let mut epsilon = false;
        let closure = self.closures_new();

        for off in 1..self.worklist.list_items_count(worklist) {
            let val = self.worklist.list_items_get(worklist, off);
            self.closures.list_items_add(closure, val & 0x7fff);
            epsilon = epsilon | (val & 0x8000 == 0x8000);
        }

        // a state where the worklist will point if successfully closed
        let next = self.worklist.list_items_get(worklist, 0);
        self.worklist.list_items_set(worklist, 0, 0);

        while changed {
            changed = false;

            for off in 0..self.closures.list_items_count(closure) {
                let src = self.closures.list_items_get(closure, off);
                if let Some((epsilon_idx, meta)) = nfa.transition_find(src, 0) {
                    for off in 0..nfa.epsilon_items_count(epsilon_idx) {
                        let val = nfa.epsilon_items_get(epsilon_idx, off);

                        if meta & 0x02 == 0x02 {
                            self.worklist_items_add(worklist, val);
                        } else {
                            if !self.closures.list_items_contains(closure, val) {
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
        }

        self.worklist.list_items_sort(worklist);
        self.worklist.list_items_distinct(worklist);

        let count = self.worklist_items_count(worklist);
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

        self.closures_items_get(closure, length)
    }

    fn nfa_iteration(&mut self, current: u16, nfa: &NFA, via: (u8, u8), dfa: &mut DFA) {
        let dst = dfa.next();
        let mut accepting = 0x00;
        let worklist = self.worklist.list_push_head();

        let size = self.worklist.list_items_count(current);
        let src = self.worklist.list_items_get(current, 0);

        // add NFA's dst state
        self.worklist.list_items_add(worklist, dst);

        // let's try to add a valid transition
        for idx in 1..size {
            let src = self.worklist.list_items_get(current, idx);
            if let Some((dst, meta)) = nfa.transition_find(src, via.0) {
                let dst = if meta & 0x02 == 0x02 { dst } else { dst | 0x8000 };
                self.worklist.list_items_add(worklist, dst);
                accepting = accepting | (meta & 0x01);
            }
        }

        // if working list contains any state
        if self.worklist.list_items_count(worklist) > 1 {
            let target = self.nfa_record_state(nfa, 0, worklist);
            dfa.transition_add(src, (via.0, via.1), target, accepting);

            if target != dst {
                dfa.next_revert();
            }
        } else {
            dfa.next_revert();
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
            self.intervals
                .list_items_add(deltas, (range & 0x00ff).rotate_left(8) | 0x01);
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

    fn nfa_to_dfa(&mut self, nfa: &NFA, dfa: &mut DFA) {
        let starting = dfa.next();
        let states = self.states_new();
        let worklist = self.worklist_new();

        // add NFA's starting state followed by DFA's starting state
        self.worklist.list_items_add(worklist, starting);
        self.worklist.list_items_add(worklist, 0);

        println!("worklist, used={}", self.worklist.usage());
        self.worklist.print();

        // the first round of epsilon discovery
        self.nfa_record_state(nfa, states, worklist);

        println!("closures, used={}", self.closures.usage());
        self.closures.print();

        while self.worklist_count() > 0 {
            println!("worklist, used={}", self.worklist.usage());
            self.worklist.print();

            // pop a list from the worklist, each worklist contains at 0 the source state id
            // the remaining items are reachable from the state id via epsilon
            let current = self.worklist.list_pop_tail();
            let intervals = self.merge_intervals(current, nfa);

            for off in 0..self.intervals.list_items_count(intervals) {
                let encoded = self.intervals.list_items_get(intervals, off);
                let (from, to) = (encoded.shr(8) as u8, (encoded & 0xff) as u8);

                self.nfa_iteration(current, nfa, (from, to), dfa);
            }

            self.intervals.list_pop_tail();

            println!("closures, used={}", self.closures.usage());
            self.closures.print();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn handles_converting_literal_regex_to_nfa() {
        let mut workbench = Workbench::new();
        let regex = Regex::Literal(b"start");
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        assert_eq!(refs, (0, 1));
        assert_eq!(nfa.transition_count(), 8);
        assert_eq!(nfa.epsilon_count(), 3);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((5, 0x00)));
        assert_eq!(nfa.epsilon_items_count(5), 1);
        assert_eq!(nfa.epsilon_items_get(5, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0, 0x02)));
        assert_eq!(nfa.epsilon_items_count(0), 1);
        assert_eq!(nfa.epsilon_items_get(0, 0), 3);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(3, b's'), Some((4, 0x02)));
        assert_eq!(nfa.transition_find(4, b't'), Some((5, 0x02)));
        assert_eq!(nfa.transition_find(5, b'a'), Some((6, 0x02)));
        assert_eq!(nfa.transition_find(6, b'r'), Some((7, 0x02)));
        assert_eq!(nfa.transition_find(7, b't'), Some((8, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((10, 0x00)));
        assert_eq!(nfa.epsilon_items_count(10), 1);
        assert_eq!(nfa.epsilon_items_get(10, 0), 1);
    }

    #[test]
    fn handles_converting_class_regex_to_nfa() {
        let mut workbench = Workbench::new();
        let regex = Regex::Class(b'0', b'9');
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        assert_eq!(refs, (0, 1));
        assert_eq!(nfa.transition_count(), 4);
        assert_eq!(nfa.epsilon_count(), 3);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((5, 0x00)));
        assert_eq!(nfa.epsilon_items_count(5), 1);
        assert_eq!(nfa.epsilon_items_get(5, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0, 0x02)));
        assert_eq!(nfa.epsilon_items_count(0), 1);
        assert_eq!(nfa.epsilon_items_get(0, 0), 3);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(3, b'0'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'1'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'2'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'3'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'4'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'5'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'6'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'7'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'8'), Some((4, 0x01)));
        assert_eq!(nfa.transition_find(3, b'9'), Some((4, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(4, 0), Some((10, 0x00)));
        assert_eq!(nfa.epsilon_items_count(10), 1);
        assert_eq!(nfa.epsilon_items_get(10, 0), 1);
    }

    #[test]
    fn handles_converting_either_regex_to_nfa() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // starting at 0 and ending at 1
        assert_eq!(refs, (0, 1));

        // 16 simple and 5 epsilon transitions are expected
        assert_eq!(nfa.transition_count(), 16);
        assert_eq!(nfa.epsilon_count(), 7);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((26, 0)));
        assert_eq!(nfa.epsilon_items_count(26), 1);
        assert_eq!(nfa.epsilon_items_get(26, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0, 0)));
        assert_eq!(nfa.epsilon_items_count(0), 2);

        assert_eq!(nfa.epsilon_items_get(0, 0), 3);
        assert_eq!(nfa.epsilon_items_get(0, 1), 10);

        // points at epsilon
        assert_eq!(nfa.transition_find(3, 0), Some((11, 0x02)));
        assert_eq!(nfa.epsilon_items_count(11), 1);
        assert_eq!(nfa.epsilon_items_get(11, 0), 4);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(4, b's'), Some((5, 0x02)));
        assert_eq!(nfa.transition_find(5, b't'), Some((6, 0x02)));
        assert_eq!(nfa.transition_find(6, b'a'), Some((7, 0x02)));
        assert_eq!(nfa.transition_find(7, b'r'), Some((8, 0x02)));
        assert_eq!(nfa.transition_find(8, b't'), Some((9, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(9, 0), Some((6, 0)));
        assert_eq!(nfa.epsilon_items_count(6), 1);
        assert_eq!(nfa.epsilon_items_get(6, 0), 16);

        // points at epsilon
        assert_eq!(nfa.transition_find(10, 0), Some((21, 0x02)));
        assert_eq!(nfa.epsilon_items_count(21), 1);
        assert_eq!(nfa.epsilon_items_get(21, 0), 11);

        // connects 'stop' literal
        assert_eq!(nfa.transition_find(11, b's'), Some((12, 0x02)));
        assert_eq!(nfa.transition_find(12, b't'), Some((13, 0x02)));
        assert_eq!(nfa.transition_find(13, b'o'), Some((14, 0x02)));
        assert_eq!(nfa.transition_find(14, b'p'), Some((15, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(15, 0), Some((16, 0)));
        assert_eq!(nfa.epsilon_items_count(16), 1);
        assert_eq!(nfa.epsilon_items_get(16, 0), 16);

        // points at epsilon
        assert_eq!(nfa.transition_find(16, 0), Some((31, 0)));
        assert_eq!(nfa.epsilon_items_count(31), 1);
        assert_eq!(nfa.epsilon_items_get(31, 0), 1);
    }

    #[test]
    fn handles_converting_concat_regex_to_nfa() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Concat(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // starting at 0 and ending at 1
        assert_eq!(refs, (0, 1));

        // 16 simple and 5 epsilon transitions are expected
        assert_eq!(nfa.transition_count(), 15);
        assert_eq!(nfa.epsilon_count(), 6);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((20, 0)));
        assert_eq!(nfa.epsilon_items_count(20), 1);
        assert_eq!(nfa.epsilon_items_get(20, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0, 0)));
        assert_eq!(nfa.epsilon_items_count(0), 1);
        assert_eq!(nfa.epsilon_items_get(0, 0), 3);

        // points at epsilon
        assert_eq!(nfa.transition_find(3, 0), Some((5, 0x02)));
        assert_eq!(nfa.epsilon_items_count(5), 1);
        assert_eq!(nfa.epsilon_items_get(5, 0), 4);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(4, b's'), Some((5, 0x02)));
        assert_eq!(nfa.transition_find(5, b't'), Some((6, 0x02)));
        assert_eq!(nfa.transition_find(6, b'a'), Some((7, 0x02)));
        assert_eq!(nfa.transition_find(7, b'r'), Some((8, 0x02)));
        assert_eq!(nfa.transition_find(8, b't'), Some((9, 0x00)));

        // points at epsilon
        assert_eq!(nfa.transition_find(9, 0), Some((10, 0)));
        assert_eq!(nfa.epsilon_items_count(10), 1);
        assert_eq!(nfa.epsilon_items_get(10, 0), 10);

        // points at epsilon
        assert_eq!(nfa.transition_find(10, 0), Some((15, 0x02)));
        assert_eq!(nfa.epsilon_items_count(15), 1);
        assert_eq!(nfa.epsilon_items_get(15, 0), 11);

        // connects 'stop' literal
        assert_eq!(nfa.transition_find(11, b's'), Some((12, 0x02)));
        assert_eq!(nfa.transition_find(12, b't'), Some((13, 0x02)));
        assert_eq!(nfa.transition_find(13, b'o'), Some((14, 0x02)));
        assert_eq!(nfa.transition_find(14, b'p'), Some((15, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(15, 0), Some((25, 0)));
        assert_eq!(nfa.epsilon_items_count(25), 1);
        assert_eq!(nfa.epsilon_items_get(25, 0), 1);
    }

    #[test]
    fn handles_converting_repeat_regex_to_nfa() {
        let mut workbench = Workbench::new();
        let regex = Regex::Literal(b"stop");
        let regex = Regex::Repeat(&regex);
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // starting at 0 and ending at 1
        assert_eq!(refs, (0, 1));

        // 5 simple transitions are expected with 3 epsilons
        assert_eq!(nfa.transition_count(), 9);
        assert_eq!(nfa.epsilon_count(), 5);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((16, 0x00)));
        assert_eq!(nfa.epsilon_items_count(16), 1);
        assert_eq!(nfa.epsilon_items_get(16, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0), 1);
        assert_eq!(nfa.epsilon_items_get(0, 0), 3);

        // points at epsilon
        assert_eq!(nfa.transition_find(3, 0), Some((11, 0x02)));
        assert_eq!(nfa.epsilon_items_count(11), 1);
        assert_eq!(nfa.epsilon_items_get(11, 0), 4);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(4, b's'), Some((5, 0x02)));
        assert_eq!(nfa.transition_find(5, b't'), Some((6, 0x02)));
        assert_eq!(nfa.transition_find(6, b'o'), Some((7, 0x02)));
        assert_eq!(nfa.transition_find(7, b'p'), Some((8, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((5, 0x00)));
        assert_eq!(nfa.epsilon_items_count(5), 2);
        assert_eq!(nfa.epsilon_items_get(5, 0), 9);
        assert_eq!(nfa.epsilon_items_get(5, 1), 2);
    }

    #[test]
    fn handles_converting_optional_regex_to_nfa() {
        let mut workbench = Workbench::new();
        let st = Regex::Literal(b"st");
        let op = Regex::Literal(b"op");
        let regex = Regex::Optional(&st);
        let regex = Regex::Concat(&regex, &op);
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // starting at 0 and ending at 1
        assert_eq!(refs, (0, 1));

        assert_eq!(nfa.transition_count(), 12);
        assert_eq!(nfa.epsilon_count(), 7);

        // points at epsilon
        assert_eq!(nfa.transition_find(0, 0), Some((26, 0x00)));
        assert_eq!(nfa.epsilon_items_count(26), 1);
        assert_eq!(nfa.epsilon_items_get(26, 0), 2);

        // points at epsilon
        assert_eq!(nfa.transition_find(2, 0), Some((0, 0x00)));
        assert_eq!(nfa.epsilon_items_count(0), 1);
        assert_eq!(nfa.epsilon_items_get(0, 0), 3);

        // points at epsilon
        assert_eq!(nfa.transition_find(3, 0), Some((5, 0x00)));
        assert_eq!(nfa.epsilon_items_count(5), 2);
        assert_eq!(nfa.epsilon_items_get(5, 0), 4);
        assert_eq!(nfa.epsilon_items_get(5, 1), 8);

        // points at epsilon
        assert_eq!(nfa.transition_find(4, 0), Some((11, 0x02)));
        assert_eq!(nfa.epsilon_items_count(11), 1);
        assert_eq!(nfa.epsilon_items_get(11, 0), 5);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(5, b's'), Some((6, 0x02)));
        assert_eq!(nfa.transition_find(6, b't'), Some((7, 0x00)));

        // points at epsilon
        assert_eq!(nfa.transition_find(7, 0), Some((5, 0x00)));
        assert_eq!(nfa.epsilon_items_count(5), 2);
        assert_eq!(nfa.epsilon_items_get(5, 0), 4);
        assert_eq!(nfa.epsilon_items_get(5, 1), 8);

        // points at epsilon
        assert_eq!(nfa.transition_find(8, 0), Some((16, 0x00)));
        assert_eq!(nfa.epsilon_items_count(16), 1);
        assert_eq!(nfa.epsilon_items_get(16, 0), 9);

        // points at epsilon
        assert_eq!(nfa.transition_find(9, 0), Some((21, 0x02)));
        assert_eq!(nfa.epsilon_items_count(21), 1);
        assert_eq!(nfa.epsilon_items_get(21, 0), 10);

        assert_eq!(nfa.transition_find(10, b'o'), Some((11, 0x02)));
        assert_eq!(nfa.transition_find(11, b'p'), Some((12, 0x01)));

        // points at epsilon
        assert_eq!(nfa.transition_find(12, 0), Some((31, 0x00)));
        assert_eq!(nfa.epsilon_items_count(31), 1);
        assert_eq!(nfa.epsilon_items_get(31, 0), 1);
    }

    #[test]
    fn handles_closing_nfa_from_epsilon_state() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);

        let mut nfa = NFA::new();
        let mut workbench = Workbench::new();

        let worklist = workbench.worklist_new();
        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // 99 is a state followed by starting state as an epsilon
        workbench.worklist_items_add(worklist, 99);
        workbench.worklist_items_add(worklist, refs.0);

        let (closure, epsilon) = workbench.nfa_close_epsilon(&nfa, worklist);

        assert_eq!(epsilon, true);
        assert_eq!(workbench.closures_items_count(closure), 3);
        assert_eq!(workbench.closures_items_get(closure, 0), 4);
        assert_eq!(workbench.closures_items_get(closure, 1), 11);

        // artificially inserted after hashing
        assert_eq!(workbench.closures_items_get(closure, 2), 99);
    }

    #[test]
    fn handles_closing_nfa_from_non_epsilon_state() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);

        let mut nfa = NFA::new();
        let mut workbench = Workbench::new();

        let worklist = workbench.worklist_new();
        let _refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // 99 is a state followed by a non-epsilon 1 state
        workbench.worklist_items_add(worklist, 99);
        workbench.worklist_items_add(worklist, 1);

        let (closure, epsilon) = workbench.nfa_close_epsilon(&nfa, worklist);

        assert_eq!(epsilon, false);
        assert_eq!(workbench.closures_items_count(closure), 2);
        assert_eq!(workbench.closures_items_get(closure, 0), 1);

        // artificially inserted after hashing
        assert_eq!(workbench.closures_items_get(closure, 1), 99);
    }

    #[test]
    fn handles_recording_nfa_state_not_repeated() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        let states = workbench.states_new();
        let worklist = workbench.worklist_new();

        workbench.worklist_items_add(worklist, refs.0);
        workbench.nfa_record_state(&nfa, states, worklist);

        // working list is not consumed, but reused
        assert_eq!(workbench.worklist_count(), 1);
    }

    #[test]
    fn handles_recording_nfa_state_repeated() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);
        let states = workbench.states_new();

        let worklist = workbench.worklist_new();
        workbench.worklist_items_add(worklist, 13);
        workbench.worklist_items_add(worklist, refs.0);
        workbench.nfa_record_state(&nfa, states, worklist);

        let worklist = workbench.worklist_new();
        workbench.worklist_items_add(worklist, 13);
        workbench.worklist_items_add(worklist, refs.0);
        workbench.nfa_record_state(&nfa, states, worklist);

        // second working list is consumed
        assert_eq!(workbench.worklist_count(), 1);
    }

    #[test]
    fn handles_converting_nfa_to_dfa_either() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.transition_count(), 7);

        assert_eq!(dfa.transition_find(0, b's'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(1, b't'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(2, b'a'), Some((3, 0x00)));
        assert_eq!(dfa.transition_find(2, b'o'), Some((4, 0x00)));
        assert_eq!(dfa.transition_find(3, b'r'), Some((5, 0x00)));
        assert_eq!(dfa.transition_find(4, b'p'), Some((6, 0x01)));
        assert_eq!(dfa.transition_find(5, b't'), Some((7, 0x01)));
    }

    #[test]
    fn handles_converting_nfa_to_dfa_concat() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Concat(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.transition_count(), 9);

        assert_eq!(dfa.transition_find(0, b's'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(1, b't'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(2, b'a'), Some((3, 0x00)));
        assert_eq!(dfa.transition_find(3, b'r'), Some((4, 0x00)));
        assert_eq!(dfa.transition_find(4, b't'), Some((5, 0x00)));
        assert_eq!(dfa.transition_find(5, b's'), Some((6, 0x00)));
        assert_eq!(dfa.transition_find(6, b't'), Some((7, 0x00)));
        assert_eq!(dfa.transition_find(7, b'o'), Some((8, 0x00)));
        assert_eq!(dfa.transition_find(8, b'p'), Some((9, 0x01)));
    }

    #[test]
    fn handles_converting_nfa_to_dfa_repeat() {
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Repeat(&stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.transition_count(), 4);

        assert_eq!(dfa.transition_find(0, b's'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(1, b't'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(2, b'o'), Some((3, 0x00)));
        assert_eq!(dfa.transition_find(3, b'p'), Some((0, 0x01)));
    }

    #[test]
    fn handles_converting_nfa_to_dfa_optional() {
        let st = Regex::Literal(b"st");
        let op = Regex::Literal(b"op");
        let regex = Regex::Optional(&st);
        let regex = Regex::Concat(&regex, &op);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.transition_count(), 4);

        assert_eq!(dfa.transition_find(0, b'o'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(0, b's'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(1, b'p'), Some((3, 0x01)));
        assert_eq!(dfa.transition_find(2, b't'), Some((0, 0x00)));
    }

    #[test]
    fn handles_converting_nfa_to_dfa_class() {
        let digits = Regex::Class(b'0', b'9');
        let regex = Regex::Repeat(&digits);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        nfa.print();
        dfa.print();

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
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.traverse(b"start", 0), Some((7, 4)));
        assert_eq!(dfa.traverse(b"stop", 0), Some((6, 3)));

        assert_eq!(dfa.traverse(b"stort", 0), None);
        assert_eq!(dfa.traverse(b"stap", 0), None);
    }

    #[test]
    fn handles_traversing_dfa_repeat() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Either(&start, &stop);
        let regex = Regex::Repeat(&regex);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.traverse(b"start", 0), Some((0, 4)));
        assert_eq!(dfa.traverse(b"stop", 0), Some((0, 3)));
        assert_eq!(dfa.traverse(b"startsta", 0), Some((0, 4)));
        assert_eq!(dfa.traverse(b"stopsto", 0), Some((0, 3)));
        assert_eq!(dfa.traverse(b"startstop", 0), Some((0, 8)));
        assert_eq!(dfa.traverse(b"stopstart", 0), Some((0, 8)));
    }

    #[test]
    fn handles_traversing_dfa_optional() {
        let start = Regex::Literal(b"start");
        let start = Regex::Optional(&start);
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Concat(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        nfa.print();
        dfa.print();

        assert_eq!(dfa.traverse(b"start", 0), None);
        assert_eq!(dfa.traverse(b"stop", 0), Some((6, 3)));
        assert_eq!(dfa.traverse(b"startsta", 0), None);
        assert_eq!(dfa.traverse(b"stopsto", 0), Some((6, 3)));
        assert_eq!(dfa.traverse(b"startstop", 0), Some((6, 8)));
        assert_eq!(dfa.traverse(b"stopstart", 0), Some((6, 3)));
    }

    #[test]
    fn handles_traversing_dfa_concat() {
        let start = Regex::Literal(b"start");
        let stop = Regex::Literal(b"stop");
        let regex = Regex::Concat(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.traverse(b"start", 0), None);
        assert_eq!(dfa.traverse(b"stop", 0), None);
        assert_eq!(dfa.traverse(b"startsta", 0), None);
        assert_eq!(dfa.traverse(b"startstop", 0), Some((9, 8)));
    }

    #[test]
    fn handles_dividing_by_three() {
        let zero = Regex::Literal(b"0");
        let one = Regex::Literal(b"1");

        let one_plus = Regex::Repeat(&one);
        let one_plus = Regex::Optional(&one_plus);

        let double = Regex::Literal(b"00");
        let double = Regex::Repeat(&double);
        let double = Regex::Optional(&double);

        let concat = Regex::Concat(&zero, &one_plus);
        let concat = Regex::Concat(&concat, &double);
        let concat = Regex::Concat(&concat, &zero);
        let concat = Regex::Repeat(&concat);
        let concat = Regex::Optional(&concat);

        let concat = Regex::Concat(&one, &concat);
        let concat = Regex::Concat(&concat, &one);
        let concat = Regex::Repeat(&concat);
        let concat = Regex::Optional(&concat);

        let either = Regex::Either(&zero, &concat);
        let either = Regex::Repeat(&either);
        let either = Regex::Optional(&either);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();
        let mut dfa = DFA::new();

        workbench.regex_to_nfa(&either, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

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
}
