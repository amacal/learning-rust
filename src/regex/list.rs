use super::alloc::Allocator;
use super::heap::AllocatorSize;
use super::heap::Guard;
use super::heap::Heap;

pub struct Collection<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u16, SIZE>> {
    heap: Heap<u16, ALLOCATOR, SIZE, GUARD>,
    count: u16,
    head: u16,
    tail: u16,
}

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u16, SIZE>> Collection<ALLOCATOR, SIZE, GUARD> {
    pub fn new(allocator: ALLOCATOR) -> Option<Self> {
        Some(Self { heap: Heap::alloc(allocator)?, count: 0, head: 0, tail: 0 })
    }

    pub fn usage(&self) -> u16 {
        let mut in_progress = true;
        let (mut idx, mut count) = (self.tail, 0u16);

        while in_progress {
            count += 4 + self.heap.get0(idx);
            idx = self.heap.get1(idx, 2);
            in_progress = idx != self.tail;
        }

        count
    }

    pub fn print(&self) {
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

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u16, SIZE>> Collection<ALLOCATOR, SIZE, GUARD> {
    pub fn list_count(&self) -> u16 {
        self.count
    }

    pub fn list_push_head(&mut self) -> u16 {
        // previous head
        let head = self.head;

        if self.count > 0 {
            // length of the current head list
            let off = self.heap.get0(self.head);

            // new head is incremented by size of the added empty list
            self.head = GUARD::apply(self.head + off + 4) as u16;
        }

        // increment number of available list
        self.count += 1;

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

    pub fn list_peak_tail(&mut self) -> u16 {
        self.tail
    }

    pub fn list_pop_tail(&mut self) -> u16 {
        // decrement number of available lists
        self.count -= 1;

        if self.count > 0 {
            // find prev and next list for the current tail
            let prev = self.list_prev(self.tail);
            let next = self.list_next(self.tail);

            // relink prev and next lists to point at each other
            self.heap.set1(prev, next, 1);
            self.heap.set1(next, prev, 2);

            // tail points where next was pointing at
            self.tail = next;
        } else {
            // reset both pointers
            self.head = 0;
            self.tail = 0;
        }

        self.tail
    }

    pub fn list_pop_head(&mut self) -> u16 {
        // decrement number of available lists
        self.count -= 1;

        if self.count > 0 {
            // find prev and next list for the current head
            let prev = self.list_prev(self.head);
            let next = self.list_next(self.head);

            // relink prev and next lists to point at each other
            self.heap.set1(prev, next, 1);
            self.heap.set1(next, prev, 2);

            // head points where prev was pointing at
            self.head = prev;
        } else {
            // reset both pointers
            self.head = 0;
            self.tail = 0;
        }

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

    pub fn list_items_count(&self, idx: u16) -> u16 {
        // count resides in the very first position
        self.heap.get0(idx)
    }

    pub fn list_items_resize(&mut self, idx: u16, size: u16) {
        // count resides in the very first position
        self.heap.set0(size, idx)
    }

    pub fn list_items_add(&mut self, idx: u16, item: u16) {
        // number of elements in the list
        let off = self.heap.get0(idx);

        // number of elements increased
        self.heap.set0(off + 1, idx);

        // new element is added where last ends plus metadata
        self.heap.set2(item, idx, off, 4);
    }

    pub fn list_items_set(&mut self, idx: u16, off: u16, item: u16) {
        // new element is replaced in-place at offset plus metadata
        self.heap.set2(item, idx, off, 4);
    }

    pub fn list_items_get(&self, idx: u16, off: u16) -> u16 {
        // element is read where it points plus metadata
        self.heap.get2(idx, off, 4)
    }

    fn list_items_put(&mut self, idx: u16, off: u16, item: u16) {
        // element is written where it points plus metadata
        self.heap.set2(item, idx, off, 4);
    }

    pub fn list_items_sort(&mut self, idx: u16) {
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

    pub fn list_items_distinct(&mut self, idx: u16) {
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

    pub fn list_items_hash(&mut self, idx: u16) {
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

    pub fn list_items_contains(&self, idx: u16, item: u16, high: u16) -> bool {
        let mut low: i32 = 0i32;
        let mut high: i32 = high.into();

        high -= 1;

        while low <= high {
            let off = low + (high - low) / 2;
            let val = self.list_items_get(idx, off as u16);

            if item == val {
                return true;
            }

            if item > val {
                low = off + 1;
            } else {
                high = off - 1;
            }
        }

        false
    }
}

impl<ALLOCATOR: Allocator, SIZE: AllocatorSize, GUARD: Guard<u16, SIZE>> Collection<ALLOCATOR, SIZE, GUARD> {
    fn set_depth(&self) -> u16 {
        let (mut idx, mut count) = (0u16, 0u16);
        let mut in_progress = true;

        while in_progress {
            count += 1;
            idx = self.heap.get1(idx, 3);
            in_progress = idx > 0;
        }

        count
    }

    fn set_capacity(&self) -> u16 {
        let (mut idx, mut count) = (0u16, 0u16);
        let mut in_progress = true;

        while in_progress {
            count += self.heap.get0(idx);
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
                    count += 1;
                }
            }

            idx = self.heap.get1(idx, 3);
            in_progress = idx > 0;
        }

        count
    }

    pub fn set_push_head(&mut self, slots: u16) -> u16 {
        // previous head
        let head = self.head;

        if self.head > 0 {
            // length of the current head list
            let off = self.heap.get0(self.head);

            // new head is incremented by size of the added empty list
            self.head = GUARD::apply(self.head + off + 4) as u16;
        }

        // increment number of available list
        self.count += 1;

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

    pub fn set_items_add(&mut self, idx: u16, list: u16) {
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

    pub fn set_items_find(&self, idx: u16, list: u16, len: u16) -> u16 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::heap::B4096;
    use super::super::heap::GuardDisabled;
    use super::super::alloc::Naive64Pages;

    fn new_collection(allocator: &Naive64Pages) -> Collection<&Naive64Pages, B4096, GuardDisabled> {
        Collection::new(allocator).unwrap()
    }

    #[test]
    fn handles_empty_data_structure() {
        let allocator = Naive64Pages::new();
        let collection = new_collection(&allocator);

        assert_eq!(collection.list_count(), 0);
    }

    #[test]
    fn handles_adding_new_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);

        let idx = collection.list_push_head();
        assert_eq!(idx, 0);

        assert_eq!(collection.list_count(), 1);
        assert_eq!(collection.list_items_count(idx), 0);

        assert_eq!(collection.list_next(idx), idx);
        assert_eq!(collection.list_prev(idx), idx);
    }

    #[test]
    fn handles_list_traversal() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);

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
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 17);

        assert_eq!(collection.list_items_count(idx), 2);
        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
    }

    #[test]
    fn handles_resizing_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_resize(idx, 2);
        assert_eq!(collection.list_items_count(idx), 2);
    }

    #[test]
    fn handles_setting_item_in_the_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_resize(idx, 2);
        collection.list_items_set(idx, 0, 13);
        collection.list_items_set(idx, 1, 17);

        assert_eq!(collection.list_items_get(idx, 0), 13);
        assert_eq!(collection.list_items_get(idx, 1), 17);
    }

    #[test]
    fn handles_removing_existing_list_from_the_head() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
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
    fn handles_removing_last_list_from_the_head() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
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

        let idx4 = collection.list_pop_head();
        assert_eq!(idx4, 0);
        assert_eq!(collection.count, 0);

        assert_eq!(collection.head, 0);
        assert_eq!(collection.tail, 0);
    }

    #[test]
    fn handles_removing_existing_list_from_the_tail() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
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
    fn handles_removing_last_list_from_the_tail() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
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

        let idx4 = collection.list_pop_tail();
        assert_eq!(idx4, 0);
        assert_eq!(collection.count, 0);

        assert_eq!(collection.head, 0);
        assert_eq!(collection.tail, 0);
    }

    #[test]
    fn handles_sorting_of_an_empty_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_sort(idx);
        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_sorting_of_single_item_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_sort(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_sorting_of_four_item_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
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
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_distinct(idx);
        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_distinct_of_single_item_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_distinct_of_list_with_zero() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 0);
        collection.list_items_add(idx, 13);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 1);
        assert_eq!(collection.list_items_get(idx, 0), 13);
    }

    #[test]
    fn handles_distinct_of_list_with_zero_only() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 0);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_distinct_of_list_with_zero_only_multiple() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 0);
        collection.list_items_add(idx, 0);
        collection.list_items_distinct(idx);

        assert_eq!(collection.list_items_count(idx), 0);
    }

    #[test]
    fn handles_distinct_of_six_item_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
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
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        assert_eq!(collection.list_items_contains(idx, 13, 0), false);
    }

    #[test]
    fn handles_finding_existing_item_in_the_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 17);
        collection.list_items_add(idx, 29);
        collection.list_items_add(idx, 31);

        assert_eq!(collection.list_items_contains(idx, 13, 4), true);
        assert_eq!(collection.list_items_contains(idx, 17, 4), true);
        assert_eq!(collection.list_items_contains(idx, 29, 4), true);
        assert_eq!(collection.list_items_contains(idx, 31, 4), true);
    }

    #[test]
    fn handles_finding_non_existing_item_in_the_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_add(idx, 13);
        collection.list_items_add(idx, 17);
        collection.list_items_add(idx, 29);
        collection.list_items_add(idx, 31);

        assert_eq!(collection.list_items_contains(idx, 14, 4), false);
        assert_eq!(collection.list_items_contains(idx, 21, 4), false);
        assert_eq!(collection.list_items_contains(idx, 27, 4), false);
        assert_eq!(collection.list_items_contains(idx, 33, 4), false);
    }

    #[test]
    fn handles_hashing_of_an_empty_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.list_push_head();

        collection.list_items_hash(idx);
        assert_eq!(collection.list_hash(idx), 0xa793u16);
    }

    #[test]
    fn handles_hashing_of_four_item_list() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
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
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);
        let idx = collection.set_push_head(8);

        assert_eq!(idx, 0);
        assert_eq!(collection.set_depth(), 1);
        assert_eq!(collection.set_count(), 0);
        assert_eq!(collection.set_capacity(), 8);
    }

    #[test]
    fn handles_adding_a_list_to_an_empty_set() {
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);

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
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);

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
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);

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
        let allocator = Naive64Pages::new();
        let mut collection = new_collection(&allocator);

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
}
