use std::ops::{Shl, Shr};

use super::alloc::Allocator;
use super::heap::AllocatorSize;
use super::heap::Guard;
use super::heap::Heap;

pub struct Graph<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u64, SIZE>> {
    heap: Heap<u64, ALLOCATOR, SIZE, GUARD>,
    head: u16,
}

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u64, SIZE>> Graph<ALLOCATOR, SIZE, GUARD> {
    pub fn new(allocator: ALLOCATOR) -> Option<Self> {
        Some(Self { heap: Heap::alloc(allocator)?, head: 0 })
    }
}

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u64, SIZE>> Graph<ALLOCATOR, SIZE, GUARD> {
    pub fn graph_count(&self) -> u16 {
        self.head
    }

    pub fn graph_inc(&mut self) -> u16 {
        let idx = self.head;
        self.head += 1;
        idx
    }

    pub fn graph_add(&mut self, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        let idx = self.graph_inc();
        self.graph_set(idx, src, via, dst, metadata);
    }

    pub fn graph_set(&mut self, idx: u16, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        let encoded = (src as u64).shl(48) | (metadata as u64).shl(16) | (dst as u64);
        let encoded = encoded | (via.0 as u64).shl(40) | (via.1 as u64).shl(32);

        self.heap.set0(encoded, idx);
    }

    pub fn graph_set_meta(&mut self, idx: u16, metadata: u16) {
        let value = self.heap.get0(idx);
        let value = value & 0xffffffff0000ffff;
        let value = value | (metadata as u64).shl(16);

        self.heap.set0(value, idx);
    }

    pub fn graph_at(&self, idx: u16) -> (u16, (u8, u8), u16, u16) {
        let val = self.heap.get0(idx);
        let src = val.shr(48) as u16;

        let via0 = (val.shr(40) & 0xffu64) as u8;
        let via1 = (val.shr(32) & 0xffu64) as u8;
        let meta = (val.shr(16) & 0xffffu64) as u16;
        let dst = (val & 0xffffu64) as u16;

        (src, (via0, via1), dst, meta)
    }

    pub fn graph_bytes(&self) -> &[u64] {
        self.heap.as_bytes(self.graph_count().into())
    }

    fn graph_swap(&mut self, left: u16, right: u16) {
        let src = self.heap.get0(left);
        let dst = self.heap.get0(right);

        self.heap.set0(dst, left);
        self.heap.set0(src, right);
    }

    fn graph_greater(&self, left: u16, right: u16) -> bool {
        let left = self.graph_at(left);
        let right = self.graph_at(right);

        left.0 > right.0 || left.0 == right.0 && left.1 .0 > right.1 .0
    }

    fn graph_heapify(&mut self, n: u16, mut i: u16) {
        loop {
            let mut largest = i;
            let left = 2 * i + 1;
            let right = 2 * i + 2;

            if left < n && self.graph_greater(left, largest) {
                largest = left;
            }

            if right < n && self.graph_greater(right, largest) {
                largest = right;
            }

            if largest != i {
                self.graph_swap(i, largest);
                i = largest;
            } else {
                break;
            }
        }
    }

    pub fn graph_sort(&mut self) {
        let n = self.head;

        // build heap
        for i in (0..(n / 2)).rev() {
            self.graph_heapify(n, i);
        }

        // one by one extract elements
        for i in (1..n).rev() {
            self.graph_swap(0, i);
            self.graph_heapify(i, 0);
        }
    }

    pub fn graph_find(&self, src: u16, via: u8) -> Option<(u16, u16)> {
        let mut low = 0i32;
        let mut high = self.head as i32;

        while low <= high {
            let idx = low + (high - low) / 2;
            let val = self.graph_at(idx as u16);

            if src == val.0 {
                if via >= val.1 .0 && via <= val.1 .1 {
                    return Some((val.2, val.3));
                }

                if via > val.1 .0 {
                    low = idx + 1;
                } else {
                    high = idx - 1;
                }
            } else {
                if src > val.0 {
                    low = idx + 1;
                } else {
                    high = idx - 1;
                }
            }
        }

        None
    }

    pub fn graph_find_all(&self, src: u16) -> Option<(u16, u16)> {
        let mut low = 0i32;
        let mut high = self.head as i32;
        let mut found = None;

        while low <= high {
            let idx = low + (high - low) / 2;
            let val = self.graph_at(idx as u16);

            if src == val.0 {
                found = Some(idx as u16);
                break;
            } else {
                if src > val.0 {
                    low = idx + 1;
                } else {
                    high = idx - 1;
                }
            }
        }

        if let Some(idx) = found {
            let (mut low, mut high) = (idx, idx);
            let (src, _, _, _) = self.graph_at(idx as u16);

            while low > 0 {
                low = match self.graph_at(low - 1) {
                    (val, _, _, _) if val == src && low > 0 => low - 1,
                    _ => break,
                };
            }

            while high < self.head {
                high = match self.graph_at(high + 1) {
                    (val, _, _, _) if val == src && high < self.head => high + 1,
                    _ => break,
                };
            }

            return Some((low, high));
        }

        None
    }

    fn graph_print(&self) {
        for idx in 0..self.head {
            let at = self.graph_at(idx);
            println!("{} ({} {}) {}", at.0, at.1 .0, at.1 .1, at.2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::heap::B4096;
    use super::super::heap::GuardDisabled;
    use super::super::alloc::Naive64Pages;

    fn new_graph(allocator: &Naive64Pages) -> Graph<&Naive64Pages, B4096, GuardDisabled> {
        Graph::new(allocator).unwrap()
    }

    #[test]
    fn handles_working_with_empty_graph() {
        let allocator = Naive64Pages::new();
        let graph = new_graph(&allocator);

        assert_eq!(graph.graph_count(), 0);
    }

    #[test]
    fn handles_adding_nodes_to_a_graph() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 99);
        graph.graph_add(29, (32, 32), 31, 98);

        assert_eq!(graph.graph_count(), 2);
        assert_eq!(graph.graph_at(0), (13, (65, 66), 17, 99));
        assert_eq!(graph.graph_at(1), (29, (32, 32), 31, 98));
    }

    #[test]
    fn handles_swapping_nodes_to_a_graph() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 99);
        graph.graph_add(29, (32, 32), 31, 98);
        graph.graph_swap(0, 1);

        assert_eq!(graph.graph_count(), 2);
        assert_eq!(graph.graph_at(0), (29, (32, 32), 31, 98));
        assert_eq!(graph.graph_at(1), (13, (65, 66), 17, 99));
    }

    #[test]
    fn handles_comparing_nodes_to_a_graph_negative() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 0);
        graph.graph_add(29, (32, 32), 31, 0);

        assert_eq!(graph.graph_greater(0, 1), false);
    }

    #[test]
    fn handles_comparing_nodes_to_a_graph_positive() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 0);
        graph.graph_add(29, (32, 32), 31, 0);

        assert_eq!(graph.graph_greater(1, 0), true);
    }

    #[test]
    fn handles_sorting_nodes_in_a_graph() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(29, (32, 32), 17, 97);
        graph.graph_add(13, (65, 66), 31, 98);
        graph.graph_add(17, (0, 0), 29, 99);
        graph.graph_sort();

        assert_eq!(graph.graph_count(), 3);
        assert_eq!(graph.graph_at(0), (13, (65, 66), 31, 98));
        assert_eq!(graph.graph_at(1), (17, (0, 0), 29, 99));
        assert_eq!(graph.graph_at(2), (29, (32, 32), 17, 97));
    }

    #[test]
    fn handles_sorting_nodes_in_a_graph_with_more_data() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        for i in 0..64 {
            graph.graph_add(13u16.wrapping_shl((7 * i) % 16), (0, 0), 0, 0);
        }

        graph.graph_sort();
        assert_eq!(graph.graph_count(), 64);

        for i in 1..64 {
            let left = graph.graph_at(i - 1);
            let right = graph.graph_at(i);

            assert!(left.0 <= right.0);
        }
    }

    #[test]
    fn handles_finding_existing_node_in_a_graph() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 97);
        graph.graph_add(17, (0, 0), 29, 98);
        graph.graph_add(29, (32, 32), 31, 99);

        assert_eq!(graph.graph_find(13, 65), Some((17, 97)));
        assert_eq!(graph.graph_find(13, 66), Some((17, 97)));
        assert_eq!(graph.graph_find(17, 0), Some((29, 98)));
        assert_eq!(graph.graph_find(29, 32), Some((31, 99)));
    }

    #[test]
    fn handles_finding_non_existing_node_in_a_graph() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 0);
        graph.graph_add(17, (0, 0), 29, 0);
        graph.graph_add(29, (32, 32), 31, 0);

        assert_eq!(graph.graph_find(12, 65), None);
        assert_eq!(graph.graph_find(13, 64), None);
        assert_eq!(graph.graph_find(13, 67), None);
        assert_eq!(graph.graph_find(17, 1), None);
        assert_eq!(graph.graph_find(29, 31), None);
        assert_eq!(graph.graph_find(29, 33), None);
        assert_eq!(graph.graph_find(30, 32), None);
    }

    #[test]
    fn handles_finding_all_existing_nodes_in_a_graph() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 97);
        graph.graph_add(17, (0, 0), 29, 98);
        graph.graph_add(29, (32, 32), 31, 99);

        assert_eq!(graph.graph_find_all(13), Some((0, 0)));
        assert_eq!(graph.graph_find_all(17), Some((1, 1)));
        assert_eq!(graph.graph_find_all(29), Some((2, 2)));
    }

    #[test]
    fn handles_finding_all_non_existing_nodes_in_a_graph() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 0);
        graph.graph_add(17, (0, 0), 29, 0);
        graph.graph_add(29, (32, 32), 31, 0);

        assert_eq!(graph.graph_find_all(12), None);
        assert_eq!(graph.graph_find_all(14), None);
        assert_eq!(graph.graph_find_all(15), None);
        assert_eq!(graph.graph_find_all(18), None);
        assert_eq!(graph.graph_find_all(28), None);
        assert_eq!(graph.graph_find_all(30), None);
    }

    #[test]
    fn handles_finding_all_existing_nodes_in_a_graph_seen_multiple_times() {
        let allocator = Naive64Pages::new();
        let mut graph = new_graph(&allocator);

        graph.graph_add(13, (65, 66), 17, 97);
        graph.graph_add(13, (90, 99), 17, 97);
        graph.graph_add(17, (0, 0), 29, 98);
        graph.graph_add(17, (99, 99), 29, 98);
        graph.graph_add(29, (32, 32), 31, 99);
        graph.graph_add(29, (33, 34), 31, 99);
        graph.graph_add(29, (36, 36), 31, 99);

        assert_eq!(graph.graph_find_all(13), Some((0, 1)));
        assert_eq!(graph.graph_find_all(17), Some((2, 3)));
        assert_eq!(graph.graph_find_all(29), Some((4, 6)));
    }
}
