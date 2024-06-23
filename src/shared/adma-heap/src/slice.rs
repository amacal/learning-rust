use super::*;

impl<'a> HeapSlice<'a> {
    pub fn ptr(&self) -> usize {
        self.src.ptr + self.off
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

impl Heap {
    pub fn between<'a>(&'a self, start: usize, end: usize) -> Result<HeapSlice<'a>, ()> {
        if start > self.len || end > self.len || start > end {
            return Err(());
        }

        let slice = HeapSlice {
            src: self,
            off: start,
            len: end - start,
        };

        Ok(slice)
    }
}
