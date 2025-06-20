pub struct InlineStack<T, const U: usize> {
    items: [T; U],
    index: usize,
}

impl<T: Copy + Default, const U: usize> InlineStack<T, U> {
    pub fn new() -> Self {
        InlineStack { items: [T::default(); U], index: 0 }
    }

    pub fn empty(&self) -> bool {
        self.index == 0
    }

    pub fn depth(&self) -> usize {
        self.index
    }

    pub fn push(&mut self, value: T) {
        unsafe {
            *self.items.get_unchecked_mut(self.index) = value;
            self.index += 1;
        }
    }

    pub fn pop(&mut self) -> T {
        unsafe {
            self.index -= 1;
            *self.items.get_unchecked(self.index)
        }
    }
}
