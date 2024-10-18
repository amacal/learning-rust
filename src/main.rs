use std::{arch::*, ops::Shr};

fn main() {
    let start = Regex::Lit(b"start");
    let stop = Regex::Lit(b"stop");
    let stopper = Regex::Lit(b"stopper");
    let regex = Regex::Or(&start, &stop);
    let regex = Regex::Or(&regex, &stopper);

    let mut workbench = Workbench::new();
    let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

    workbench.regex_to_nfa(&regex, &mut nfa);

    println!("nfa, transitions={}", nfa.transition_count());
    nfa.transition_sort();
    nfa.print();

    workbench.nfa_to_dfa(&nfa, &mut dfa);
}

enum Regex<'a> {
    Lit(&'a [u8]),
    Or(&'a Regex<'a>, &'a Regex<'a>),
}

struct Heap<T, const SIZE: usize>(*mut T);

impl<T, const SIZE: usize> Heap<T, SIZE> {
    fn alloc() -> Self {
        let ptr = unsafe {
            let ret: isize;

            asm!(
                "syscall",
                in("rax") 9,
                in("rdi") 0,
                in("rsi") SIZE,
                in("rdx") 0x00000001 | 0x00000002,
                in("r10") 0x00000002 | 0x00000020,
                in("r8") 0,
                in("r9") 0,
                lateout("rcx") _,
                lateout("r11") _,
                lateout("rax") ret,
                options(nostack)
            );

            ret
        };

        Self(ptr as *mut T)
    }

    fn mask(&self) -> usize {
        SIZE / size_of::<T>() - 1
    }

    fn round0(&self, off: usize) -> usize {
        (off & self.mask()) as usize
    }

    fn round1(&self, off: usize, inc: usize) -> usize {
        (off.wrapping_add(inc) & self.mask()) as usize
    }

    fn round2(&self, off: usize, inc1: usize, inc2: usize) -> usize {
        (off.wrapping_add(inc1).wrapping_add(inc2) & self.mask()) as usize
    }

    fn get0<U>(&self, off: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        unsafe { *self.0.add(self.round0(off.into())) }
    }

    fn set0<U>(&self, val: T, off: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        unsafe { *self.0.add(self.round0(off.into())) = val }
    }

    fn get1<U>(&self, off: U, inc: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        unsafe { *self.0.add(self.round1(off.into(), inc.into())) }
    }

    fn set1<U>(&self, val: T, off: U, inc: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        unsafe { *self.0.add(self.round1(off.into(), inc.into())) = val }
    }

    fn get2<U>(&self, off: U, inc1: U, inc2: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        unsafe { *self.0.add(self.round2(off.into(), inc1.into(), inc2.into())) }
    }

    fn set2<U>(&self, val: T, off: U, inc1: U, inc2: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        unsafe { *self.0.add(self.round2(off.into(), inc1.into(), inc2.into())) = val }
    }
}

impl<T, const SIZE: usize> Drop for Heap<T, SIZE> {
    fn drop(&mut self) {
        unsafe {
            asm!(
                "syscall",
                in("rax") 11,
                in("rdi") self.0,
                in("rsi") SIZE,
                lateout("rcx") _,
                lateout("r11") _,
                lateout("rax") _,
                options(nostack)
            );
        }
    }
}

struct Collection<const SIZE: usize> {
    heap: Heap<u16, SIZE>,
    count: u16,
    head: u16,
    tail: u16,
}

impl<const SIZE: usize> Collection<SIZE> {
    fn new() -> Self {
        unsafe {
            let heap = Heap::alloc();
            let ptr = heap.0 as *mut u16;

            // simulate that current list has some elements
            // so that new list will start at index zero
            *ptr = (SIZE / 2) as u16 - 4;

            Self {
                heap: heap,
                count: 0,
                head: 0,
                tail: 0,
            }
        }
    }

    fn usage(&self) -> u16 {
        let mut in_progress = true;
        let (mut idx, mut count) = (self.tail, 0u16);

        while in_progress {
            count = count.wrapping_add(4);
            count = count.wrapping_add(self.heap.get0(idx));
            idx = self.heap.get1(idx, 2);
            in_progress = idx != self.tail;
        }

        count
    }

    fn print(&self) {
        let mut idx = self.tail;

        loop {
            let cnt = self.heap.get0(idx);
            let prev = self.heap.get1(idx, 1);
            let next = self.heap.get1(idx, 2);
            let hash = self.heap.get1(idx, 3);

            print!("{:04x} | {:04x} {:04x} {:04x} {:04x} | ", idx, cnt, prev, next, hash);

            for off in 0..cnt {
                print!("{:04x} ", self.heap.get1(idx, 4 + off));
            }

            println!();

            if next == self.tail {
                break;
            }

            idx = next;
        }

        println!();
    }
}

impl<const SIZE: usize> Collection<SIZE> {
    fn list_count(&self) -> u16 {
        self.count
    }

    fn list_push_head(&mut self) -> u16 {
        // previous head
        let head = self.head;

        // length of the current head list
        let off = self.heap.get0(self.head);

        // increment number of available list
        self.count = self.count.wrapping_add(1);

        // new head is incremented by size of the added empty list
        self.head = self.heap.round2(self.head.into(), off.into(), 4) as u16;

        // new list contains zero elements and no hash
        self.heap.set0(0, self.head);
        self.heap.set1(0, self.head, 3);

        // new list points back at previous list
        self.heap.set1(head, self.head, 1);

        // new list points head at global tail
        self.heap.set1(self.tail, self.head, 2);

        // previous list points next at newly created list
        self.heap.set1(self.head, head, 2);

        // global tail list points back at newly created list
        self.heap.set1(self.head, self.tail, 1);

        // updated head pointing at newly created list
        self.head
    }

    fn list_pop_tail(&mut self) -> u16 {
        // decrement number of available lists
        self.count = self.count.wrapping_sub(1);

        // find prev and next list for the current tail
        let prev = self.list_prev(self.tail);
        let next = self.list_next(self.tail);

        // relink prev and next lists to point at each other
        self.heap.set1(prev, next, 1);
        self.heap.set1(next, prev, 2);

        // tail points where next was pointing at
        self.tail = next;
        self.tail
    }

    fn list_pop_head(&mut self) -> u16 {
        // decrement number of available lists
        self.count = self.count.wrapping_sub(1);

        // find prev and next list for the current head
        let prev = self.list_prev(self.head);
        let next = self.list_next(self.head);

        // relink prev and next lists to point at each other
        self.heap.set1(prev, next, 1);
        self.heap.set1(next, prev, 2);

        // head points where prev was pointing at
        self.head = prev;
        self.head
    }

    fn list_prev(&self, idx: u16) -> u16 {
        // simply find next list
        self.heap.get1(idx, 1)
    }

    fn list_next(&self, idx: u16) -> u16 {
        // simply find next list
        self.heap.get1(idx, 2)
    }

    fn list_hash(&self, idx: u16) -> u16 {
        // simply find hash of the list
        self.heap.get1(idx, 3)
    }

    fn list_items_count(&self, idx: u16) -> u16 {
        // count resides in the very first position
        self.heap.get0(idx)
    }

    fn list_items_resize(&self, idx: u16, size: u16) {
        // count resides in the very first position
        self.heap.set0(size, idx)
    }

    fn list_items_add(&mut self, idx: u16, item: u16) {
        // number of elements in the list
        let off = self.heap.get0(idx);

        // number of elements increased
        self.heap.set0(off + 1, idx);

        // new element is added where last ends plus metadata
        self.heap.set2(item, idx, off, 4);
    }

    fn list_items_set(&mut self, idx: u16, off: u16, item: u16) {
        // new element is replaced in-place at offset plus metadata
        self.heap.set2(item, idx, off, 4);
    }

    fn list_items_get(&self, idx: u16, off: u16) -> u16 {
        // element is read where it points plus metadata
        self.heap.get2(idx, off, 4)
    }

    fn list_items_put(&self, idx: u16, off: u16, item: u16) {
        // element is written where it points plus metadata
        self.heap.set2(item, idx, off, 4);
    }

    fn list_items_sort(&mut self, idx: u16) {
        for i in 1..self.list_items_count(idx) {
            let key = self.list_items_get(idx, i);
            let mut j = (i - 1) as isize;

            while j >= 0 && self.list_items_get(idx, j as u16) > key {
                let val = self.list_items_get(idx, j as u16);
                self.list_items_put(idx, j as u16 + 1, val);

                j = j - 1;
            }

            self.list_items_put(idx, (j + 1) as u16, key);
        }
    }

    fn list_items_distinct(&mut self, idx: u16) {
        // first find a number of items
        let count = self.list_items_count(idx);

        if count > 1 {
            // writing head is at 0 and first value is assumed to be unique
            let mut write = 0;
            let mut prev = self.list_items_get(idx, write);

            for read in 1..count {
                // fetch value from the reading head
                let current = self.list_items_get(idx, read);

                // if the item is not equal to current writing head - it's unique
                if current != prev {
                    write = write + 1;
                    prev = current;

                    // and store the value at the right position
                    self.list_items_put(idx, write, current);
                }
            }

            // the count needs to be updated
            self.heap.set0(write + 1, idx);
        }
    }

    fn list_items_hash(&mut self, idx: u16) {
        let mut hash = 0xc6a4a793u32;

        for off in 0..self.list_items_count(idx) {
            let val = self.list_items_get(idx, off) as u32;

            hash = hash.wrapping_shr(16);
            hash ^= val.wrapping_mul(0xc6a4a793u32);
        }

        self.heap.set1((hash & 0xffff) as u16, idx, 3);
    }

    fn list_items_contains(&self, idx: u16, item: u16) -> bool {
        let mut low = 0i32;
        let mut high = self.list_items_count(idx) as i32;

        while low <= high {
            let off = low + (high - low) / 2;
            let val = self.list_items_get(idx, off as u16);

            if item == val {
                return true;
            }

            if item > val {
                low = off.wrapping_add(1);
            } else {
                high = off.wrapping_sub(1);
            }
        }

        false
    }
}

impl<const SIZE: usize> Collection<SIZE> {
    fn set_depth(&self) -> u16 {
        let (mut idx, mut count) = (0u16, 0u16);
        let mut in_progress = true;

        while in_progress {
            count = count.wrapping_add(1);
            idx = self.heap.get1(idx, 3);
            in_progress = idx > 0;
        }

        count
    }

    fn set_capacity(&self) -> u16 {
        let (mut idx, mut count) = (0u16, 0u16);
        let mut in_progress = true;

        while in_progress {
            count = count.wrapping_add(self.heap.get0(idx));
            idx = self.heap.get1(idx, 3);
            in_progress = idx > 0;
        }

        count
    }

    fn set_count(&self) -> u16 {
        let (mut idx, mut count) = (0u16, 0u16);
        let mut in_progress = true;

        while in_progress {
            for i in 0..self.heap.get0(idx) {
                if self.heap.get2(idx, i, 4) != 0 {
                    count = count.wrapping_add(1);
                }
            }

            idx = self.heap.get1(idx, 3);
            in_progress = idx > 0;
        }

        count
    }

    fn set_push_head(&mut self, slots: u16) -> u16 {
        // previous head
        let head = self.head;

        // length of the current head list
        let off = self.heap.get0(self.head);

        // increment number of available list
        self.count = self.count.wrapping_add(1);

        // new head is incremented by size of the added empty list
        self.head = self.heap.round2(self.head as usize, off as usize, 4) as u16;

        // new list contains number of passed slots and no link
        self.heap.set0(slots, self.head);
        self.heap.set1(0, self.head, 3);

        // all slots are zeroed
        for i in 0..slots {
            self.heap.set2(0, self.head, i, 4);
        }

        // new list points back at previous list
        self.heap.set1(head, self.head, 1);

        // new list points head at global tail
        self.heap.set1(self.tail, self.head, 2);

        // previous list points next at newly created list
        self.heap.set1(self.head, head, 2);

        // global tail list points back at newly created list
        self.heap.set1(self.head, self.tail, 1);

        self.head
    }

    fn set_items_add(&mut self, idx: u16, list: u16) {
        let mut idx = idx;

        // hash behind the list
        let hash = self.heap.get1(list, 3);

        loop {
            // count behind the set
            let count = self.heap.get0(idx);
            let off = self.heap.round1((hash & (count - 1)).into(), 4);

            // ptr to slot for the list
            let slot = self.heap.get1(idx, off as u16);

            // slot is found, just return
            if slot == 0 {
                self.heap.set1(list, idx, off as u16);
                return;
            }

            // let's try to find it in the next set
            let next = self.heap.get1(idx, 3);
            if next != 0 {
                idx = next;
                continue;
            }

            // no set available, let's create new one
            let next = self.set_push_head(2 * count);
            self.heap.set1(next, idx, 3);

            idx = next;
        }
    }

    fn set_items_find(&self, idx: u16, list: u16) -> u16 {
        let mut idx = idx;
        let mut in_progress = true;

        // hash behind the list
        let hash = self.heap.get1(list, 3);

        while in_progress {
            // count behind the set
            let count = self.heap.get0(idx);

            // slot for the list, aka list idx
            let slot = self.heap.get2(idx, hash & (count - 1), 4);

            // if slot if not taken list was surely not found
            if slot == 0 {
                break;
            }

            // if hash behind found slot doesn't match, list was not found in this round
            if self.heap.get1(slot, 3) != hash {
                idx = self.heap.get1(idx, 3);
                in_progress = idx > 0;
                continue;
            }

            // if count behind found slot doesn't match, list was not found in this round
            if self.heap.get0(slot) != self.heap.get0(list) {
                idx = self.heap.get1(idx, 3);
                in_progress = idx > 0;
                continue;
            }

            // check item by item
            let mut found = true;
            for i in 0..self.heap.get0(list) {
                let left = self.heap.get2(list, i, 4);
                let right = self.heap.get2(slot, i, 4);

                if left != right {
                    found = false;
                    continue;
                }
            }

            // values were not rejected
            if found {
                return slot;
            }

            // let's try to find it in the next round
            idx = self.heap.get1(idx, 3);
            in_progress = idx > 0;
        }

        0
    }
}

impl<const SIZE: usize> Collection<SIZE> {
    fn graph_count(&self) -> u16 {
        self.head
    }

    fn graph_add(&mut self, src: u16, via: (u8, u8), dst: u16) {
        // update the head
        let idx = self.head;
        self.head = self.head.wrapping_add(1);

        // the first word stores the source node
        self.heap.set0(src, idx.rotate_left(3));

        // the second word stores combined via pair
        self.heap
            .set1(((via.0 as u16) << 8) + via.1 as u16, idx.rotate_left(3), 1);

        // the third word stores the destination
        self.heap.set1(dst, idx.rotate_left(3), 2);

        // the fourh word will store some metadata
        self.heap.set1(0, idx.rotate_left(3), 3);
    }

    fn graph_at(&self, idx: u16) -> (u16, (u8, u8), u16) {
        let src = self.heap.get0(idx.rotate_left(3));
        let via = self.heap.get1(idx.rotate_left(3), 1);
        let dst = self.heap.get1(idx.rotate_left(3), 2);

        (src, (via.shr(8) as u8, (via & 0xff) as u8), dst)
    }

    fn graph_swap(&mut self, left: u16, right: u16) {
        unsafe {
            let src = self.heap.round0(left.rotate_left(3).into()).shr(2);
            let dst = self.heap.round0(right.rotate_left(3).into()).shr(2);

            let ptr = self.heap.0 as *mut u64;
            let tmp = *ptr.add(src);

            *ptr.add(src) = *ptr.add(dst);
            *ptr.add(dst) = tmp;
        }
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

    fn graph_sort(&mut self) {
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

    fn graph_find(&self, src: u16, via: u8) -> Option<u16> {
        let mut low = 0i32;
        let mut high = self.head as i32;

        while low <= high {
            let idx = low + (high - low) / 2;
            let val = self.graph_at(idx as u16);

            if src == val.0 {
                if via >= val.1 .0 && via <= val.1 .1 {
                    return Some(val.2);
                }

                if via > val.1 .0 {
                    low = idx.wrapping_add(1);
                } else {
                    high = idx.wrapping_sub(1);
                }
            } else {
                if src > val.0 {
                    low = idx.wrapping_add(1);
                } else {
                    high = idx.wrapping_sub(1);
                }
            }
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

struct NFA {
    counter: u16,
    transitions: Collection<4096>,
    epsilons: Collection<4096>,
}

impl NFA {
    fn new() -> Self {
        Self {
            counter: 0,
            transitions: Collection::new(),
            epsilons: Collection::new(),
        }
    }

    fn next(&mut self) -> u16 {
        self.counter = self.counter.wrapping_add(1);
        self.counter.wrapping_sub(1)
    }

    fn transition_count(&self) -> u16 {
        self.transitions.graph_count()
    }

    fn transition_at(&self, idx: u16) -> (u16, (u8, u8), u16) {
        self.transitions.graph_at(idx)
    }

    fn transition_find(&self, src: u16, via: u8) -> Option<u16> {
        self.transitions.graph_find(src, via)
    }

    fn transition_add(&mut self, src: u16, via: (u8, u8), dst: u16) {
        self.transitions.graph_add(src, via, dst)
    }

    fn transition_sort(&mut self) {
        self.transitions.graph_sort()
    }

    fn epsilon_new(&mut self) -> u16 {
        self.epsilons.list_push_head()
    }

    fn epsilon_count(&self) -> u16 {
        self.epsilons.list_count()
    }

    fn epsilon_items_resize(&self, idx: u16, size: u16) {
        self.epsilons.list_items_resize(idx, size)
    }

    fn epsilon_items_add(&mut self, idx: u16, item: u16) {
        self.epsilons.list_items_add(idx, item)
    }

    fn epsilon_items_set(&mut self, idx: u16, off: u16, item: u16) {
        self.epsilons.list_items_set(idx, off, item)
    }

    fn epsilon_items_get(&self, idx: u16, off: u16) -> u16 {
        self.epsilons.list_items_get(idx, off)
    }

    fn epsilon_items_sort(&mut self, idx: u16) {
        self.epsilons.list_items_sort(idx)
    }

    fn epsilon_items_distinct(&mut self, idx: u16) {
        self.epsilons.list_items_distinct(idx)
    }

    fn epsilon_items_count(&self, idx: u16) -> u16 {
        self.epsilons.list_items_count(idx)
    }

    fn print(&self) {
        for idx in 0..self.transition_count() {
            let transition = self.transition_at(idx);
            print!("{:04x} | {:02x} - {:02x} | ", transition.0, transition.1 .0, transition.1 .1);

            if transition.1 .0 > 0 {
                println!("{:04x}", transition.2);
            } else {
                for off in 0..self.epsilon_items_count(transition.2) {
                    print!("{:04x} ", self.epsilon_items_get(transition.2, off))
                }

                println!();
            }
        }

        println!();
    }
}

struct DFA {
    counter: u16,
    transitions: Collection<4096>,
}

impl DFA {
    fn new() -> Self {
        Self {
            counter: 0,
            transitions: Collection::new(),
        }
    }

    fn next(&mut self) -> u16 {
        self.counter = self.counter.wrapping_add(1);
        self.counter.wrapping_sub(1)
    }

    fn next_revert(&mut self) {
        self.counter = self.counter.wrapping_sub(1);
    }

    fn transition_at(&self, idx: u16) -> (u16, (u8, u8), u16) {
        return self.transitions.graph_at(idx);
    }

    fn transition_add(&mut self, src: u16, via: (u8, u8), dst: u16) {
        self.transitions.graph_add(src, via, dst)
    }

    fn transition_count(&self) -> u16 {
        self.transitions.graph_count()
    }

    fn transition_find(&self, src: u16, via: u8) -> Option<u16> {
        self.transitions.graph_find(src, via)
    }
}

struct Matrix {
    heap: Heap<u16, 4096>,
}

impl Matrix {
    fn new() -> Self {
        Self { heap: Heap::alloc() }
    }

    fn set(&mut self, src: u16, via: (u8, u8), dst: u16) {
        unsafe {
            let ptr = self.heap.0;
            let base = src.wrapping_shl(8);

            for i in via.0..=via.1 {
                println!("{} {} {}", src, i, dst);
                *ptr.add(base.wrapping_add(i as u16) as usize) = dst;
            }
        }
    }

    fn traverse(&self, data: &[u8]) -> (u16, usize) {
        unsafe {
            let mut state = 0u16;
            let ptr = self.heap.0;

            for (idx, &val) in data.iter().enumerate() {
                let base = state.wrapping_shl(8);
                let base = base.wrapping_add(val as u16);

                state = match *ptr.add(base as usize) {
                    0 => return (state, idx),
                    1 => return (1, idx + 1),
                    state => state,
                };
            }

            return (state, data.len());
        }
    }
}

struct Workbench {
    worklist: Collection<4096>,
    closures: Collection<4096>,
}

impl Workbench {
    fn new() -> Self {
        Self {
            worklist: Collection::new(),
            closures: Collection::new(),
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

    fn regex_to_nfa(&mut self, regex: &Regex, nfa: &mut NFA) -> (u16, u16) {
        fn into_nfa(node: &Regex, nfa: &mut NFA) -> (u16, u16) {
            match node {
                Regex::Lit(value) => {
                    let first = nfa.next();
                    let mut last = first;

                    for &val in value.iter() {
                        let next = nfa.next();
                        nfa.transition_add(last, (val, val), next);
                        last = next;
                    }

                    (first, last)
                }
                Regex::Or(left, right) => {
                    let first = nfa.next();
                    let last = nfa.next();

                    // transitions should be only added in an increasing order
                    let ep1 = nfa.epsilon_new();
                    nfa.transition_add(first, (0, 0), ep1);
                    nfa.epsilon_items_resize(ep1, 2);

                    let ep2 = nfa.epsilon_new();
                    nfa.epsilon_items_add(ep2, last);

                    let ep3 = nfa.epsilon_new();
                    nfa.epsilon_items_add(ep3, last);

                    let left = into_nfa(left, nfa);
                    nfa.epsilon_items_set(ep1, 0, left.0);
                    nfa.transition_add(left.1, (0, 0), ep2);

                    let right = into_nfa(right, nfa);
                    nfa.epsilon_items_set(ep1, 1, right.0);
                    nfa.transition_add(right.1, (0, 0), ep3);

                    (first, last)
                }
            }
        }

        into_nfa(regex, nfa)
    }

    fn nfa_close_epsilon(&mut self, nfa: &NFA, worklist: u16) -> u16 {
        let mut changed = true;
        let closure = self.closures_new();

        for off in 1..self.worklist.list_items_count(worklist) {
            let val = self.worklist.list_items_get(worklist, off);
            self.closures.list_items_add(closure, val);
        }

        while changed {
            changed = false;

            for off in 0..self.closures.list_items_count(closure) {
                let src = self.closures.list_items_get(closure, off);
                if let Some(epsilon) = nfa.transition_find(src, 0) {
                    for off in 0..nfa.epsilon_items_count(epsilon) {
                        let val = nfa.epsilon_items_get(epsilon, off);
                        if !self.closures.list_items_contains(closure, val) {
                            self.closures.list_items_add(closure, val);
                            changed = true;
                        }
                    }
                }
            }

            if changed {
                self.closures.list_items_sort(closure);
                self.closures.list_items_distinct(closure);
            }
        }

        self.closures.list_items_hash(closure);
        closure
    }

    fn nfa_record_state(&mut self, nfa: &NFA, states: u16, worklist: u16) {
        // find all transition via epsilon and find potentially available idx
        let closure = self.nfa_close_epsilon(nfa, worklist);
        let idx = self.closures.set_items_find(states, closure);

        if idx > 0 {
            // closure is useless and working list needs to be consumed from head
            self.closures.list_pop_head();
            self.worklist.list_pop_head();
        } else {
            // the next item will be picked from the current working list
            self.worklist.list_items_resize(worklist, 1);

            // closure is added to the set
            self.closures.set_items_add(states, closure);

            // entire closure is copied to a working list
            for off in 0..self.closures.list_items_count(closure) {
                let val = self.closures.list_items_get(closure, off);
                self.worklist.list_items_add(worklist, val);
            }
        }
    }

    fn nfa_to_dfa(&mut self, nfa: &NFA, dfa: &mut DFA) {
        let next = dfa.next();
        let accepting = dfa.next();

        let states = self.states_new();
        let worklist = self.worklist_new();

        self.worklist.list_items_add(worklist, next);
        self.worklist.list_items_add(worklist, 0);
        self.nfa_record_state(nfa, states, worklist);

        while self.worklist_count() > 0 {
            println!("worklist, used={}", self.worklist.usage());
            self.worklist.print();

            let current = self.worklist.list_pop_tail();
            let src = self.worklist.list_items_get(current, 0);

            for via in 1..=255 {
                // next DFA transition and new working list
                let dst = dfa.next();
                let worklist = self.worklist.list_push_head();
                self.worklist.list_items_add(worklist, dst);

                // let's try to add a valid transition
                for idx in 1..self.worklist.list_items_count(current) {
                    let src = self.worklist.list_items_get(current, idx);
                    if let Some(dst) = nfa.transition_find(src, via) {
                        self.worklist.list_items_add(worklist, dst);
                    }
                }

                // if working list contains any state
                if self.worklist.list_items_count(worklist) > 1 {
                    self.nfa_record_state(nfa, states, worklist);

                    let dst = if self.worklist.list_items_get(worklist, 1) == 1 {
                        dfa.next_revert();
                        accepting
                    } else {
                        dst
                    };

                    println!("set {src} {via} {dst}");
                    dfa.transition_add(src, (via, via), dst);
                } else {
                    dfa.next_revert();
                    self.worklist.list_pop_head();
                }
            }

            println!("closures, used={}", self.closures.usage());
            self.closures.print();
        }
    }

    fn dfa_to_matrix(&self, dfa: &DFA, matrix: &mut Matrix) {
        for idx in 0..dfa.transition_count() {
            let val = dfa.transition_at(idx);
            matrix.set(val.0, val.1, val.2);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Collection, Matrix, Regex, Workbench, DFA, NFA};

    #[test]
    fn handles_empty_data_structure() {
        let collection = Collection::<4096>::new();

        assert_eq!(collection.list_count(), 0);
    }

    #[test]
    fn handles_adding_new_list() {
        let mut collection = Collection::<4096>::new();

        let idx = collection.list_push_head();
        assert_eq!(idx, 0);

        assert_eq!(collection.list_count(), 1);
        assert_eq!(collection.list_items_count(idx), 0);

        assert_eq!(collection.list_next(idx), idx);
        assert_eq!(collection.list_prev(idx), idx);
    }

    #[test]
    fn handles_list_traversal() {
        let mut collection = Collection::<4096>::new();
        let idx1 = collection.list_push_head();
        let idx2 = collection.list_push_head();
        let idx3 = collection.list_push_head();

        assert_eq!(collection.list_next(idx1), idx2);
        assert_eq!(collection.list_next(idx2), idx3);
        assert_eq!(collection.list_next(idx3), idx1);

        assert_eq!(collection.list_prev(idx1), idx3);
        assert_eq!(collection.list_prev(idx3), idx2);
        assert_eq!(collection.list_prev(idx2), idx1);
    }

    #[test]
    fn handles_adding_item_to_the_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 17);

        assert_eq!(collection.list_items_count(idx), 2);
        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
    }

    #[test]
    fn handles_resizing_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_resize(idx, 2);
        assert_eq!(collection.list_items_count(idx), 2);
    }

    #[test]
    fn handles_setting_item_in_the_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_resize(idx, 2);
        collection.list_items_set(idx, 0, 13);
        collection.list_items_set(idx, 1, 17);

        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
    }

    #[test]
    fn handles_removing_existing_list_from_the_head() {
        let mut collection = Collection::<4096>::new();
        let idx1 = collection.list_push_head();

        collection.list_items_add(idx1, 13);
        collection.list_items_add(idx1, 17);
        assert_eq!(collection.list_items_count(idx1), 2);

        let idx2 = collection.list_push_head();
        collection.list_items_add(idx2, 23);
        assert_eq!(collection.list_items_get(idx2, 0), 23);

        let idx3 = collection.list_pop_head();
        assert_eq!(idx3, idx1);
        assert_eq!(collection.list_items_get(idx3, 0), 13);
        assert_eq!(collection.list_items_get(idx3, 1), 17);

        assert_eq!(collection.list_prev(idx1), idx1);
        assert_eq!(collection.list_next(idx1), idx1);
    }

    #[test]
    fn handles_removing_existing_list_from_the_tail() {
        let mut collection = Collection::<4096>::new();
        let idx1 = collection.list_push_head();

        collection.list_items_add(idx1, 13);
        collection.list_items_add(idx1, 17);
        assert_eq!(collection.list_items_count(idx1), 2);

        let idx2 = collection.list_push_head();
        collection.list_items_add(idx2, 23);
        assert_eq!(collection.list_items_get(idx2, 0), 23);

        let idx3 = collection.list_pop_tail();
        assert_eq!(idx3, idx2);
        assert_eq!(collection.list_items_get(idx3, 0), 23);

        assert_eq!(collection.list_prev(idx2), idx2);
        assert_eq!(collection.list_next(idx2), idx2);
    }

    #[test]
    fn handles_sorting_of_an_empty_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_sort(idx);
        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_sorting_of_single_item_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_sort(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_sorting_of_four_item_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 31);
        collection.list_items_add(idx, 29);
        collection.list_items_add(idx, 17);

        collection.list_items_sort(idx);
        assert_eq!(collection.list_items_count(idx), 4);

        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
        assert_eq!(collection.list_items_get(idx, 2), 29);
        assert_eq!(collection.list_items_get(idx, 3), 31);
    }

    #[test]
    fn handles_distinct_of_an_empty_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_distinct(idx);
        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_distinct_of_single_item_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_distinct_of_six_item_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 31);
        collection.list_items_add(idx, 29);
        collection.list_items_add(idx, 17);
        collection.list_items_add(idx, 29);
        collection.list_items_add(idx, 13);

        collection.list_items_sort(idx);
        assert_eq!(collection.list_items_count(idx), 6);

        collection.list_items_distinct(idx);
        assert_eq!(collection.list_items_count(idx), 4);

        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
        assert_eq!(collection.list_items_get(idx, 2), 29);
        assert_eq!(collection.list_items_get(idx, 3), 31);
    }

    #[test]
    fn handles_finding_item_in_an_empty_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        assert_eq!(collection.list_items_contains(idx, 13), false);
    }

    #[test]
    fn handles_finding_existing_item_in_the_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 17);
        collection.list_items_add(idx, 29);
        collection.list_items_add(idx, 31);

        assert_eq!(collection.list_items_contains(idx, 13), true);
        assert_eq!(collection.list_items_contains(idx, 17), true);
        assert_eq!(collection.list_items_contains(idx, 29), true);
        assert_eq!(collection.list_items_contains(idx, 31), true);
    }

    #[test]
    fn handles_finding_non_existing_item_in_the_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 17);
        collection.list_items_add(idx, 29);
        collection.list_items_add(idx, 31);

        assert_eq!(collection.list_items_contains(idx, 14), false);
        assert_eq!(collection.list_items_contains(idx, 21), false);
        assert_eq!(collection.list_items_contains(idx, 27), false);
        assert_eq!(collection.list_items_contains(idx, 33), false);
    }

    #[test]
    fn handles_hashing_of_an_empty_list() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.list_push_head();

        collection.list_items_hash(idx);
        assert_eq!(collection.list_hash(idx), 0xa793u16);
    }

    #[test]
    fn handles_hashing_of_four_item_list() {
        let mut collection = Collection::<4096>::new();
        let idx1 = collection.list_push_head();

        collection.list_items_add(idx1, 13);
        collection.list_items_add(idx1, 31);
        collection.list_items_add(idx1, 29);
        collection.list_items_add(idx1, 17);

        collection.list_items_hash(idx1);
        assert_ne!(collection.list_hash(idx1), 0);

        let idx2 = collection.list_push_head();

        collection.list_items_add(idx2, 13);
        collection.list_items_add(idx2, 31);
        collection.list_items_add(idx2, 29);
        collection.list_items_add(idx2, 17);

        collection.list_items_hash(idx2);
        assert_ne!(collection.list_hash(idx2), 0);

        assert_eq!(collection.list_hash(idx1), collection.list_hash(idx2));
    }

    #[test]
    fn handles_adding_new_set() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.set_push_head(8);

        assert_eq!(idx, 0);
        assert_eq!(collection.set_depth(), 1);
        assert_eq!(collection.set_count(), 0);
        assert_eq!(collection.set_capacity(), 8);
    }

    #[test]
    fn handles_adding_a_list_to_an_empty_set() {
        let mut collection = Collection::<4096>::new();

        let idx1 = collection.set_push_head(8);
        let idx2 = collection.list_push_head();

        collection.list_items_add(idx2, 13);
        collection.list_items_add(idx2, 17);
        collection.list_items_add(idx2, 29);
        collection.list_items_add(idx2, 31);

        collection.list_items_hash(idx2);
        collection.set_items_add(idx1, idx2);

        assert_eq!(collection.set_depth(), 1);
        assert_eq!(collection.set_count(), 1);
        assert_eq!(collection.set_capacity(), 8);
    }

    #[test]
    fn handles_finding_existing_element_in_the_set() {
        let mut collection = Collection::<4096>::new();

        let idx1 = collection.set_push_head(8);
        let idx2 = collection.list_push_head();

        collection.list_items_add(idx2, 13);
        collection.list_items_add(idx2, 17);
        collection.list_items_add(idx2, 29);
        collection.list_items_add(idx2, 31);

        collection.list_items_hash(idx2);
        collection.set_items_add(idx1, idx2);

        let idx3 = collection.list_push_head();

        collection.list_items_add(idx3, 13);
        collection.list_items_add(idx3, 17);
        collection.list_items_add(idx3, 29);
        collection.list_items_add(idx3, 31);
        collection.list_items_hash(idx3);

        let idx4 = collection.set_items_find(idx1, idx3);
        assert_eq!(idx4, idx2);
        assert_ne!(idx4, idx3);
    }

    #[test]
    fn handles_finding_non_existing_element_in_the_set() {
        let mut collection = Collection::<4096>::new();

        let idx1 = collection.set_push_head(8);
        let idx2 = collection.list_push_head();

        collection.list_items_add(idx2, 13);
        collection.list_items_add(idx2, 17);
        collection.list_items_add(idx2, 29);
        collection.list_items_add(idx2, 31);

        collection.list_items_hash(idx2);
        collection.set_items_add(idx1, idx2);

        let idx3 = collection.list_push_head();

        collection.list_items_add(idx3, 13);
        collection.list_items_add(idx3, 17);
        collection.list_items_add(idx3, 29);
        collection.list_items_add(idx3, 37);
        collection.list_items_hash(idx3);

        let idx4 = collection.set_items_find(idx1, idx3);
        assert_eq!(idx4, 0);
        assert_ne!(idx4, idx3);
    }

    #[test]
    fn handles_finding_bunch_of_items_in_the_set() {
        let mut collection = Collection::<4096>::new();
        let idx = collection.set_push_head(8);
        let mut lists = [0; 16];

        for i in 0..lists.len() {
            lists[i] = collection.list_push_head();

            for j in 0..i + 1 {
                collection.list_items_add(lists[i], j as u16);
            }

            collection.list_items_hash(lists[i]);
            collection.set_items_add(idx, lists[i]);
        }

        for i in 0..lists.len() {
            println!("{i}");
            assert_ne!(collection.set_items_find(idx, lists[i]), 0);
        }

        assert_eq!(collection.set_depth(), 2);
        assert_eq!(collection.set_count(), 16);
        assert_eq!(collection.set_capacity(), 24);
    }

    #[test]
    fn handles_working_with_empty_graph() {
        let collection = Collection::<4096>::new();
        assert_eq!(collection.graph_count(), 0);
    }

    #[test]
    fn handles_adding_nodes_to_a_graph() {
        let mut collection = Collection::<4096>::new();

        collection.graph_add(13, (65, 66), 17);
        collection.graph_add(29, (32, 32), 31);

        assert_eq!(collection.graph_count(), 2);
        assert_eq!(collection.graph_at(0), (13, (65, 66), 17));
        assert_eq!(collection.graph_at(1), (29, (32, 32), 31));
    }

    #[test]
    fn handles_swapping_nodes_to_a_graph() {
        let mut collection = Collection::<4096>::new();

        collection.graph_add(13, (65, 66), 17);
        collection.graph_add(29, (32, 32), 31);
        collection.graph_swap(0, 1);

        assert_eq!(collection.graph_count(), 2);
        assert_eq!(collection.graph_at(0), (29, (32, 32), 31));
        assert_eq!(collection.graph_at(1), (13, (65, 66), 17));
    }

    #[test]
    fn handles_comparing_nodes_to_a_graph_negative() {
        let mut collection = Collection::<4096>::new();

        collection.graph_add(13, (65, 66), 17);
        collection.graph_add(29, (32, 32), 31);

        assert_eq!(collection.graph_greater(0, 1), false);
    }

    #[test]
    fn handles_comparing_nodes_to_a_graph_positive() {
        let mut collection = Collection::<4096>::new();

        collection.graph_add(13, (65, 66), 17);
        collection.graph_add(29, (32, 32), 31);

        assert_eq!(collection.graph_greater(1, 0), true);
    }

    #[test]
    fn handles_sorting_nodes_in_a_graph() {
        let mut collection = Collection::<4096>::new();

        collection.graph_add(29, (32, 32), 17);
        collection.graph_add(13, (65, 66), 31);
        collection.graph_add(17, (0, 0), 29);
        collection.graph_sort();

        assert_eq!(collection.graph_count(), 3);
        assert_eq!(collection.graph_at(0), (13, (65, 66), 31));
        assert_eq!(collection.graph_at(1), (17, (0, 0), 29));
        assert_eq!(collection.graph_at(2), (29, (32, 32), 17));
    }

    #[test]
    fn handles_sorting_nodes_in_a_graph_with_more_data() {
        let mut collection = Collection::<4096>::new();

        for i in 0..64 {
            collection.graph_add(13u16.wrapping_shl((7 * i) % 16), (0, 0), 0);
        }

        collection.graph_sort();
        assert_eq!(collection.graph_count(), 64);

        for i in 1..64 {
            let left = collection.graph_at(i - 1);
            let right = collection.graph_at(i);

            assert!(left.0 <= right.0);
        }
    }

    #[test]
    fn handles_finding_existing_node_in_a_graph() {
        let mut collection = Collection::<4096>::new();

        collection.graph_add(13, (65, 66), 17);
        collection.graph_add(17, (0, 0), 29);
        collection.graph_add(29, (32, 32), 31);

        assert_eq!(collection.graph_find(13, 65), Some(17));
        assert_eq!(collection.graph_find(13, 66), Some(17));
        assert_eq!(collection.graph_find(17, 0), Some(29));
        assert_eq!(collection.graph_find(29, 32), Some(31));
    }

    #[test]
    fn handles_finding_non_existing_node_in_a_graph() {
        let mut collection = Collection::<4096>::new();

        collection.graph_add(13, (65, 66), 17);
        collection.graph_add(17, (0, 0), 29);
        collection.graph_add(29, (32, 32), 31);

        assert_eq!(collection.graph_find(12, 65), None);
        assert_eq!(collection.graph_find(13, 64), None);
        assert_eq!(collection.graph_find(13, 67), None);
        assert_eq!(collection.graph_find(17, 1), None);
        assert_eq!(collection.graph_find(29, 31), None);
        assert_eq!(collection.graph_find(29, 33), None);
        assert_eq!(collection.graph_find(30, 32), None);
    }

    #[test]
    fn handles_converting_literal_regex_to_nfa() {
        let mut workbench = Workbench::new();
        let regex = Regex::Lit(b"start");
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // starting at 0 and ending at 5
        assert_eq!(refs, (0, 5));

        // only 5 simple transitions are expected
        assert_eq!(nfa.transition_count(), 5);
        assert_eq!(nfa.epsilon_count(), 0);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(0, b's'), Some(1));
        assert_eq!(nfa.transition_find(1, b't'), Some(2));
        assert_eq!(nfa.transition_find(2, b'a'), Some(3));
        assert_eq!(nfa.transition_find(3, b'r'), Some(4));
        assert_eq!(nfa.transition_find(4, b't'), Some(5));
    }

    #[test]
    fn handles_converting_or_regex_to_nfa() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // starting at 0 and ending at 1
        assert_eq!(refs, (0, 1));

        // 9 simple and 3 epsilon transitions are expected
        assert_eq!(nfa.transition_count(), 12);
        assert_eq!(nfa.epsilon_count(), 3);

        // 0 points at epsilon pointing at 2 and 8
        assert_eq!(nfa.transition_find(0, 0), Some(0));
        assert_eq!(nfa.epsilon_items_count(0), 2);

        assert_eq!(nfa.epsilon_items_get(0, 0), 2);
        assert_eq!(nfa.epsilon_items_get(0, 1), 8);

        // connects 'start' literal
        assert_eq!(nfa.transition_find(2, b's'), Some(3));
        assert_eq!(nfa.transition_find(3, b't'), Some(4));
        assert_eq!(nfa.transition_find(4, b'a'), Some(5));
        assert_eq!(nfa.transition_find(5, b'r'), Some(6));
        assert_eq!(nfa.transition_find(6, b't'), Some(7));

        // 6 points at epsilon pointing at 1
        assert_eq!(nfa.transition_find(7, 0), Some(6));
        assert_eq!(nfa.epsilon_items_count(6), 1);
        assert_eq!(nfa.epsilon_items_get(6, 0), 1);

        // connects 'stop' literal
        assert_eq!(nfa.transition_find(8, b's'), Some(9));
        assert_eq!(nfa.transition_find(9, b't'), Some(10));
        assert_eq!(nfa.transition_find(10, b'o'), Some(11));
        assert_eq!(nfa.transition_find(11, b'p'), Some(12));

        // 12 points at epsilon pointing at 1
        assert_eq!(nfa.transition_find(12, 0), Some(11));
        assert_eq!(nfa.epsilon_items_count(11), 1);
        assert_eq!(nfa.epsilon_items_get(11, 0), 1);
    }

    #[test]
    fn handles_closing_nfa_from_epsilon_state() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let worklist = workbench.worklist_new();
        let refs = workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.worklist_items_add(worklist, 13);
        workbench.worklist_items_add(worklist, refs.0);

        let closure = workbench.nfa_close_epsilon(&nfa, worklist);
        assert_eq!(workbench.closures_items_count(closure), 3);
        assert_eq!(workbench.closures_items_get(closure, 0), 0);
        assert_eq!(workbench.closures_items_get(closure, 1), 2);
        assert_eq!(workbench.closures_items_get(closure, 2), 8);
    }

    #[test]
    fn handles_closing_nfa_from_non_epsilon_state() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let worklist = workbench.worklist_new();
        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.worklist_items_add(worklist, 13);
        workbench.worklist_items_add(worklist, 1);

        let closure = workbench.nfa_close_epsilon(&nfa, worklist);
        assert_eq!(workbench.closures_items_count(closure), 1);
        assert_eq!(workbench.closures_items_get(closure, 0), 1);
    }

    #[test]
    fn handles_recording_nfa_state_not_repeated() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

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
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        let states = workbench.states_new();
        let worklist = workbench.worklist_new();

        workbench.worklist_items_add(worklist, 13);
        workbench.worklist_items_add(worklist, refs.0);

        workbench.nfa_record_state(&nfa, states, worklist);
        workbench.nfa_record_state(&nfa, states, worklist);

        // working list is consumed
        assert_eq!(workbench.worklist_count(), 0);
    }

    #[test]
    fn handles_converting_nfa_to_dfa() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        assert_eq!(dfa.transition_count(), 7);

        assert_eq!(dfa.transition_find(0, b's'), Some(2));
        assert_eq!(dfa.transition_find(2, b't'), Some(3));
        assert_eq!(dfa.transition_find(3, b'a'), Some(4));
        assert_eq!(dfa.transition_find(3, b'o'), Some(5));
        assert_eq!(dfa.transition_find(4, b'r'), Some(6));
        assert_eq!(dfa.transition_find(5, b'p'), Some(1));
        assert_eq!(dfa.transition_find(6, b't'), Some(1));
    }

    #[test]
    fn handles_traversing_dfa_positive() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());
        let mut matrix = Matrix::new();

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);
        workbench.dfa_to_matrix(&dfa, &mut matrix);

        // expect final state 1 at 5th character
        assert_eq!(matrix.traverse(b"start"), (1, 5));

        // expect final state 1 at 4th character
        assert_eq!(matrix.traverse(b"stop"), (1, 4));
    }

    #[test]
    fn handles_traversing_dfa_negative() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());
        let mut matrix = Matrix::new();

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);
        workbench.dfa_to_matrix(&dfa, &mut matrix);

        // expect failed state 5 at 3th character, because of 'r'
        assert_eq!(matrix.traverse(b"stort"), (5, 3));

        // expect failed state 4 at 3th character, because of 'p'
        assert_eq!(matrix.traverse(b"stap"), (4, 3));
    }
}
