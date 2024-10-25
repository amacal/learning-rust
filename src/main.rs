use std::{arch::*, marker::PhantomData, ops::Shr, ptr};

fn main() {
    let start = Regex::Lit(b"start");
    let stop = Regex::Lit(b"stop");
    let stopper = Regex::Lit(b"stopper");
    let regex = Regex::Or(&start, &stop);
    let regex = Regex::Or(&regex, &stopper);
    let regex = Regex::Rep(&regex);

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
    Lit(&'a [u8]),
    Or(&'a Regex<'a>, &'a Regex<'a>),
    Rep(&'a Regex<'a>),
}

trait Guard<T, const SIZE: usize> {
    fn apply(off: usize) -> usize;

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy;

    fn deref_set(ptr: *mut T, off: usize, val: T);
}

struct GuardWrapping;
struct GuardDisabled;
struct GuardSegfault;

impl<T, const SIZE: usize> Guard<T, SIZE> for GuardWrapping {
    fn apply(off: usize) -> usize {
        off & (SIZE / size_of::<T>() - 1)
    }

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy,
    {
        unsafe { *ptr.add(off) }
    }

    fn deref_set(ptr: *mut T, off: usize, val: T) {
        unsafe { *ptr.add(off) = val }
    }
}

impl<T, const SIZE: usize> Guard<T, SIZE> for GuardDisabled {
    fn apply(off: usize) -> usize {
        off
    }

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy,
    {
        unsafe { *ptr.add(off) }
    }

    fn deref_set(ptr: *mut T, off: usize, val: T) {
        unsafe { *ptr.add(off) = val }
    }
}

impl<T, const SIZE: usize> Guard<T, SIZE> for GuardSegfault {
    fn apply(off: usize) -> usize {
        off
    }

    fn deref_get(ptr: *const T, off: usize) -> T
    where
        T: Copy,
    {
        unsafe {
            let src = if off < SIZE / size_of::<T>() {
                ptr.add(off)
            } else {
                ptr::null()
            };

            ptr::read_volatile(src)
        }
    }

    fn deref_set(ptr: *mut T, off: usize, val: T) {
        unsafe {
            let dst = if off < SIZE / size_of::<T>() {
                ptr.add(off)
            } else {
                ptr::null_mut()
            };

            ptr::write_volatile(dst, val);
        }
    }
}

struct Heap<T, const SIZE: usize, GUARD: Guard<T, SIZE>>(*mut T, PhantomData<GUARD>);

impl<T, const SIZE: usize, GUARD: Guard<T, SIZE>> Heap<T, SIZE, GUARD> {
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

        Self(ptr as *mut T, PhantomData)
    }

    fn deref_get(&self, off: usize) -> T
    where
        T: Copy,
    {
        GUARD::deref_get(self.0, off)
    }

    fn deref_set(&mut self, off: usize, val: T) {
        GUARD::deref_set(self.0, off, val);
    }

    fn guard0(&self, off: usize) -> usize {
        GUARD::apply(off)
    }

    fn guard1(&self, off: usize, inc: usize) -> usize {
        GUARD::apply(off.wrapping_add(inc))
    }

    fn guard2(&self, off: usize, inc1: usize, inc2: usize) -> usize {
        GUARD::apply(off.wrapping_add(inc1).wrapping_add(inc2))
    }

    fn get0<U>(&self, off: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_get(self.guard0(off.into()))
    }

    fn set0<U>(&mut self, val: T, off: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_set(self.guard0(off.into()), val)
    }

    fn get1<U>(&self, off: U, inc: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_get(self.guard1(off.into(), inc.into()))
    }

    fn set1<U>(&mut self, val: T, off: U, inc: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_set(self.guard1(off.into(), inc.into()), val);
    }

    fn get2<U>(&self, off: U, inc1: U, inc2: U) -> T
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_get(self.guard2(off.into(), inc1.into(), inc2.into()))
    }

    fn set2<U>(&mut self, val: T, off: U, inc1: U, inc2: U)
    where
        T: Copy,
        U: Into<usize>,
    {
        self.deref_set(self.guard2(off.into(), inc1.into(), inc2.into()), val);
    }
}

impl<T, const SIZE: usize, GUARD: Guard<T, SIZE>> Drop for Heap<T, SIZE, GUARD> {
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

struct Collection<const SIZE: usize, GUARD: Guard<u16, SIZE>> {
    heap: Heap<u16, SIZE, GUARD>,
    count: u16,
    head: u16,
    tail: u16,
}

impl<const SIZE: usize, GUARD: Guard<u16, SIZE>> Collection<SIZE, GUARD> {
    fn new() -> Self {
        let mut heap = Heap::alloc();

        // simulate that current list has some elements
        // so that new list will start at index zero
        heap.set0((SIZE / 2) as u16 - 4, 0u16);

        Self {
            heap: heap,
            count: 0,
            head: 0,
            tail: 0,
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

                if off % 8 == 7 && off + 1 < cnt {
                    print!("\n                           | ");
                }
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

impl<const SIZE: usize, GUARD: Guard<u16, SIZE>> Collection<SIZE, GUARD> {
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
        self.head =
            <GuardWrapping as Guard<u16, SIZE>>::apply(self.head.wrapping_add(off).wrapping_add(4).into()) as u16;

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

    fn list_items_resize(&mut self, idx: u16, size: u16) {
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

    fn list_items_put(&mut self, idx: u16, off: u16, item: u16) {
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

        if count > 0 {
            // writing head is at 0
            let mut write = 0;
            let mut off = 0;
            let mut prev = 0;

            for read in 0..count {
                prev = self.list_items_get(idx, read);
                if prev > 0 {
                    off = read + 1;
                    write = write + 1;
                    self.list_items_set(idx, 0, prev);
                    break;
                }
            }

            for read in off..count {
                // fetch value from the reading head
                let current = self.list_items_get(idx, read);

                // if the item is not equal to current writing head - it's unique
                if current != prev {
                    // and store the value at the right position
                    self.list_items_put(idx, write, current);

                    write = write + 1;
                    prev = current;
                }
            }

            // the count needs to be updated
            self.heap.set0(write, idx);
        }
    }

    fn list_items_hash(&mut self, idx: u16) {
        // a value taken from murmur hash
        let mut hash = 0xc6a4a793u32;

        for off in 0..self.list_items_count(idx) {
            let val: u32 = self.list_items_get(idx, off).into();

            // a murmur like hashing
            hash = hash.rotate_right(16);
            hash ^= val.wrapping_mul(0xc6a4a793u32);
        }

        // update to metadata
        self.heap.set1((hash & 0xffff) as u16, idx, 3);
    }

    fn list_items_contains(&self, idx: u16, item: u16) -> bool {
        let mut low: i32 = 0i32;
        let mut high: i32 = self.list_items_count(idx).into();

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

impl<const SIZE: usize, GUARD: Guard<u16, SIZE>> Collection<SIZE, GUARD> {
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
        self.head =
            <GuardWrapping as Guard<u16, SIZE>>::apply(self.head.wrapping_add(off).wrapping_add(4).into()) as u16;

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
        let mut depth = 0;

        loop {
            // hash behind the list
            let hash = self.heap.get1(list, 3);
            let shift = hash.rotate_left(depth % 32);

            // count behind the set
            let count = self.heap.get0(idx);
            let off = self.heap.guard1((shift & (count - 1)).into(), 4);

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
                depth += 1;
                continue;
            }

            // no set available, let's create new one
            let next = self.set_push_head(2 * count);
            self.heap.set1(next, idx, 3);

            idx = next;
            depth += 1;
        }
    }

    fn set_items_find(&self, idx: u16, list: u16, len: u16) -> u16 {
        let mut idx = idx;
        let mut depth = 0;
        let mut in_progress = true;

        while in_progress {
            // hash behind the list
            let hash = self.heap.get1(list, 3);
            let shift = hash.rotate_left(depth % 32);

            // count behind the set
            let count = self.heap.get0(idx);

            // slot for the list, aka list idx
            let slot = self.heap.get2(idx, shift & (count - 1), 4);

            // if slot if not taken list was surely not found
            if slot == 0 {
                break;
            }

            // if hash behind found slot doesn't match, list was not found in this round
            if self.heap.get1(slot, 3) != hash {
                idx = self.heap.get1(idx, 3);
                in_progress = idx > 0;
                depth += 1;
                continue;
            }

            // if count behind found slot doesn't match, list was not found in this round
            if self.heap.get0(slot) != self.heap.get0(list) {
                idx = self.heap.get1(idx, 3);
                in_progress = idx > 0;
                depth += 1;
                continue;
            }

            // check item by item
            let mut found = true;
            for i in 0..std::cmp::min(len, self.heap.get0(list)) {
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
            depth += 1;
        }

        0
    }
}

struct Graph<const SIZE: usize, GUARD: Guard<u16, SIZE>> {
    heap: Heap<u16, SIZE, GUARD>,
    head: u16,
}

impl<const SIZE: usize, GUARD: Guard<u16, SIZE>> Graph<SIZE, GUARD> {
    fn new() -> Self {
        Self {
            heap: Heap::alloc(),
            head: 0,
        }
    }
}

impl<const SIZE: usize, GUARD: Guard<u16, SIZE>> Graph<SIZE, GUARD> {
    fn graph_count(&self) -> u16 {
        self.head
    }

    fn graph_inc(&mut self) -> u16 {
        let idx = self.head;
        self.head = self.head.wrapping_add(1);
        idx
    }

    fn graph_add(&mut self, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        // first increment the index
        let idx = self.graph_inc();

        // then write at it
        self.graph_set(idx, src, via, dst, metadata);
    }

    fn graph_set(&mut self, idx: u16, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        // the first word stores the source node
        self.heap.set0(src, idx.rotate_left(3));

        // the second word stores combined via pair
        self.heap
            .set1(((via.0 as u16) << 8) + via.1 as u16, idx.rotate_left(3), 1);

        // the third word stores the destination
        self.heap.set1(dst, idx.rotate_left(3), 2);

        // the fourh word will store some metadata
        self.heap.set1(metadata, idx.rotate_left(3), 3);
    }

    fn graph_at(&self, idx: u16) -> (u16, (u8, u8), u16, u16) {
        let src = self.heap.get0(idx.rotate_left(3));
        let via = self.heap.get1(idx.rotate_left(3), 1);
        let dst = self.heap.get1(idx.rotate_left(3), 2);
        let meta = self.heap.get1(idx.rotate_left(3), 3);

        (src, (via.shr(8) as u8, (via & 0xff) as u8), dst, meta)
    }

    fn graph_swap(&mut self, left: u16, right: u16) {
        unsafe {
            let src = self.heap.guard0(left.rotate_left(3).into()).shr(2);
            let dst = self.heap.guard0(right.rotate_left(3).into()).shr(2);

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

    fn graph_find(&self, src: u16, via: u8) -> Option<(u16, u16)> {
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
    transitions: Graph<4096, GuardDisabled>,
    epsilons: Collection<4096, GuardDisabled>,
}

impl NFA {
    fn new() -> Self {
        Self {
            counter: 0,
            transitions: Graph::new(),
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

    fn transition_at(&self, idx: u16) -> (u16, (u8, u8), u16, u16) {
        self.transitions.graph_at(idx)
    }

    fn transition_find(&self, src: u16, via: u8) -> Option<(u16, u16)> {
        self.transitions.graph_find(src, via)
    }

    fn transition_inc(&mut self) -> u16 {
        self.transitions.graph_inc()
    }

    fn transition_add(&mut self, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        self.transitions.graph_add(src, via, dst, metadata);
    }

    fn transition_set(&mut self, idx: u16, src: u16, via: (u8, u8), dst: u16, metadata: u16) {
        self.transitions.graph_set(idx, src, via, dst, metadata);
    }

    fn transition_sort(&mut self) {
        self.transitions.graph_sort();
    }

    fn epsilon_new(&mut self) -> u16 {
        self.epsilons.list_push_head()
    }

    fn epsilon_count(&self) -> u16 {
        self.epsilons.list_count()
    }

    fn epsilon_items_resize(&mut self, idx: u16, size: u16) {
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
            print!(
                "{:04x} | {:02x} - {:02x} | {:04x} | ",
                transition.0, transition.1 .0, transition.1 .1, transition.3
            );

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

    fn traverse(&self, data: &[u8], start: u16) -> (u16, usize) {
        let mut current = (start, 0);
        let mut best = None;

        for (idx, &val) in data.iter().enumerate() {
            current = match self.transition_find(current.0, val) {
                None => break,
                Some((state, meta)) => {
                    if meta & 0x01 == 0x01 {
                        best = Some((state, idx + 1));
                    }

                    (state, idx)
                }
            };
        }

        best.unwrap_or(current)
    }
}

struct Workbench {
    worklist: Collection<4096, GuardWrapping>,
    closures: Collection<4096, GuardDisabled>,
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

    fn worklist_items_count(&mut self, idx: u16) -> u16 {
        self.worklist.list_items_count(idx)
    }

    fn regex_to_nfa(&mut self, regex: &Regex, nfa: &mut NFA) -> (u16, u16) {
        fn into_nfa(node: &Regex, nfa: &mut NFA, accepting: bool) -> (u16, u16) {
            match node {
                Regex::Lit(value) => {
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
                Regex::Or(left, right) => {
                    let first = nfa.next();

                    // transitions should be only added in an increasing order
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
                Regex::Rep(target) => {
                    let first = nfa.next();
                    let ep1 = nfa.epsilon_new();

                    nfa.transition_add(first, (0, 0), ep1, 0);
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
            let src = self.worklist.list_items_get(current, 0);

            for via in 1..=255 {
                // next DFA transition and new working list
                let dst = dfa.next();
                let mut accepting = 0x00;
                let worklist = self.worklist.list_push_head();

                // add NFA's dst state
                self.worklist.list_items_add(worklist, dst);

                // let's try to add a valid transition
                for idx in 1..self.worklist.list_items_count(current) {
                    let src = self.worklist.list_items_get(current, idx);
                    if let Some((dst, meta)) = nfa.transition_find(src, via) {
                        let dst = if meta & 0x02 == 0x02 { dst } else { dst | 0x8000 };
                        self.worklist.list_items_add(worklist, dst);
                        accepting = accepting | (meta & 0x01);
                    }
                }

                // if working list contains any state
                if self.worklist.list_items_count(worklist) > 1 {
                    let target = self.nfa_record_state(nfa, states, worklist);
                    dfa.transition_add(src, (via, via), target, accepting);

                    if target != dst {
                        dfa.next_revert();
                    }
                } else {
                    dfa.next_revert();
                    self.worklist.list_pop_head();
                }
            }

            println!("closures, used={}", self.closures.usage());
            self.closures.print();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn handles_empty_data_structure() {
        let collection = Collection::<4096, GuardDisabled>::new();

        assert_eq!(collection.list_count(), 0);
    }

    #[test]
    fn handles_adding_new_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();

        let idx = collection.list_push_head();
        assert_eq!(idx, 0);

        assert_eq!(collection.list_count(), 1);
        assert_eq!(collection.list_items_count(idx), 0);

        assert_eq!(collection.list_next(idx), idx);
        assert_eq!(collection.list_prev(idx), idx);
    }

    #[test]
    fn handles_list_traversal() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 17);

        assert_eq!(collection.list_items_count(idx), 2);
        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
    }

    #[test]
    fn handles_resizing_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_resize(idx, 2);
        assert_eq!(collection.list_items_count(idx), 2);
    }

    #[test]
    fn handles_setting_item_in_the_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_resize(idx, 2);
        collection.list_items_set(idx, 0, 13);
        collection.list_items_set(idx, 1, 17);

        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
    }

    #[test]
    fn handles_removing_existing_list_from_the_head() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_sort(idx);
        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_sorting_of_single_item_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_sort(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_sorting_of_four_item_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_distinct(idx);
        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_distinct_of_single_item_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_distinct_of_list_with_zero() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 0);
        collection.list_items_add(idx, 13);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_distinct_of_list_with_zero_only() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 0);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_distinct_of_list_with_zero_only_multiple() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 0);
        collection.list_items_add(idx, 0);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_distinct_of_six_item_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        assert_eq!(collection.list_items_contains(idx, 13), false);
    }

    #[test]
    fn handles_finding_existing_item_in_the_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.list_push_head();

        collection.list_items_hash(idx);
        assert_eq!(collection.list_hash(idx), 0xa793u16);
    }

    #[test]
    fn handles_hashing_of_four_item_list() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
        let mut collection = Collection::<4096, GuardDisabled>::new();
        let idx = collection.set_push_head(8);

        assert_eq!(idx, 0);
        assert_eq!(collection.set_depth(), 1);
        assert_eq!(collection.set_count(), 0);
        assert_eq!(collection.set_capacity(), 8);
    }

    #[test]
    fn handles_adding_a_list_to_an_empty_set() {
        let mut collection = Collection::<4096, GuardDisabled>::new();

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
        let mut collection = Collection::<4096, GuardDisabled>::new();

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

        let idx4 = collection.set_items_find(idx1, idx3, 4);
        assert_eq!(idx4, idx2);
        assert_ne!(idx4, idx3);
    }

    #[test]
    fn handles_finding_non_existing_element_in_the_set() {
        let mut collection = Collection::<4096, GuardDisabled>::new();

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

        let idx4 = collection.set_items_find(idx1, idx3, 4);
        assert_eq!(idx4, 0);
        assert_ne!(idx4, idx3);
    }

    #[test]
    fn handles_finding_bunch_of_items_in_the_set() {
        let mut collection = Collection::<4096, GuardDisabled>::new();
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
            assert_ne!(collection.set_items_find(idx, lists[i], i as u16 + 1), 0);
        }

        assert_eq!(collection.set_depth(), 4);
        assert_eq!(collection.set_count(), 16);
        assert_eq!(collection.set_capacity(), 120);
    }

    #[test]
    fn handles_working_with_empty_graph() {
        let graph = Graph::<4096, GuardDisabled>::new();
        assert_eq!(graph.graph_count(), 0);
    }

    #[test]
    fn handles_adding_nodes_to_a_graph() {
        let mut graph = Graph::<4096, GuardDisabled>::new();

        graph.graph_add(13, (65, 66), 17, 99);
        graph.graph_add(29, (32, 32), 31, 98);

        assert_eq!(graph.graph_count(), 2);
        assert_eq!(graph.graph_at(0), (13, (65, 66), 17, 99));
        assert_eq!(graph.graph_at(1), (29, (32, 32), 31, 98));
    }

    #[test]
    fn handles_swapping_nodes_to_a_graph() {
        let mut graph = Graph::<4096, GuardDisabled>::new();

        graph.graph_add(13, (65, 66), 17, 99);
        graph.graph_add(29, (32, 32), 31, 98);
        graph.graph_swap(0, 1);

        assert_eq!(graph.graph_count(), 2);
        assert_eq!(graph.graph_at(0), (29, (32, 32), 31, 98));
        assert_eq!(graph.graph_at(1), (13, (65, 66), 17, 99));
    }

    #[test]
    fn handles_comparing_nodes_to_a_graph_negative() {
        let mut graph = Graph::<4096, GuardDisabled>::new();

        graph.graph_add(13, (65, 66), 17, 0);
        graph.graph_add(29, (32, 32), 31, 0);

        assert_eq!(graph.graph_greater(0, 1), false);
    }

    #[test]
    fn handles_comparing_nodes_to_a_graph_positive() {
        let mut graph = Graph::<4096, GuardDisabled>::new();

        graph.graph_add(13, (65, 66), 17, 0);
        graph.graph_add(29, (32, 32), 31, 0);

        assert_eq!(graph.graph_greater(1, 0), true);
    }

    #[test]
    fn handles_sorting_nodes_in_a_graph() {
        let mut graph = Graph::<4096, GuardDisabled>::new();

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
        let mut graph = Graph::<4096, GuardDisabled>::new();

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
        let mut graph = Graph::<4096, GuardDisabled>::new();

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
        let mut graph = Graph::<4096, GuardDisabled>::new();

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
    fn handles_converting_literal_regex_to_nfa() {
        let mut workbench = Workbench::new();
        let regex = Regex::Lit(b"start");
        let mut nfa = NFA::new();

        let refs = workbench.regex_to_nfa(&regex, &mut nfa);

        // starting at 0 and ending at 1
        assert_eq!(refs, (0, 1));

        // 5 simple transitions are expected with 3 epsilons
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
    fn handles_closing_nfa_from_epsilon_state() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

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
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

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

        let worklist = workbench.worklist_new();
        workbench.worklist_items_add(worklist, 13);
        workbench.worklist_items_add(worklist, refs.0);
        workbench.nfa_record_state(&nfa, states, worklist);

        // second working list is consumed
        assert_eq!(workbench.worklist_count(), 1);
    }

    #[test]
    fn handles_converting_nfa_to_dfa_or() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        nfa.print();
        dfa.print();

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
    fn handles_converting_nfa_to_dfa_rep() {
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Rep(&stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);
        dfa.print();
        nfa.print();

        assert_eq!(dfa.transition_count(), 4);

        assert_eq!(dfa.transition_find(0, b's'), Some((1, 0x00)));
        assert_eq!(dfa.transition_find(1, b't'), Some((2, 0x00)));
        assert_eq!(dfa.transition_find(2, b'o'), Some((3, 0x00)));
        assert_eq!(dfa.transition_find(3, b'p'), Some((0, 0x01)));
    }

    #[test]
    fn handles_traversing_dfa_or_positive() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        // expect final state 7 at 5th character
        assert_eq!(dfa.traverse(b"start", 0), (7, 5));

        // expect final state 6 at 4th character
        assert_eq!(dfa.traverse(b"stop", 0), (6, 4));
    }

    #[test]
    fn handles_traversing_dfa_or_negative() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        // expect failed state 4 at 2nd character, because of 'r'
        assert_eq!(dfa.traverse(b"stort", 0), (4, 2));

        // expect failed state 3 at 2nd character, because of 'p'
        assert_eq!(dfa.traverse(b"stap", 0), (3, 2));
    }

    #[test]
    fn handles_traversing_dfa_rep_positive() {
        let start = Regex::Lit(b"start");
        let stop = Regex::Lit(b"stop");
        let regex = Regex::Or(&start, &stop);
        let regex = Regex::Rep(&regex);

        let mut workbench = Workbench::new();
        let (mut nfa, mut dfa) = (NFA::new(), DFA::new());

        workbench.regex_to_nfa(&regex, &mut nfa);
        workbench.nfa_to_dfa(&nfa, &mut dfa);

        nfa.print();
        dfa.print();

        // expect final state 7 at 5th character
        assert_eq!(dfa.traverse(b"start", 0), (0, 5));

        // expect final state 6 at 4th character
        assert_eq!(dfa.traverse(b"stop", 0), (0, 4));

        // expect final state 7 at 5th character
        assert_eq!(dfa.traverse(b"startsta", 0), (0, 5));

        // expect final state 6 at 4th character
        assert_eq!(dfa.traverse(b"stopsto", 0), (0, 4));

        // expect final state 19 at 9th character
        assert_eq!(dfa.traverse(b"startstop", 0), (0, 9));

        // expect final state 20 at 9th character
        assert_eq!(dfa.traverse(b"stopstart", 0), (0, 9));
    }
}
