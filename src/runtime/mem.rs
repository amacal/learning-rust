use crate::heap::*;
use crate::uring::*;

impl IORingSubmitBuffer for &Droplet<Heap> {
    fn extract(self) -> (*const u8, usize) {
        (self.ptr as *const u8, self.len)
    }
}

impl IORingSubmitBuffer for &HeapSlice {
    fn extract(self) -> (*const u8, usize) {
        (self.ptr as *const u8, self.len)
    }
}

impl IORingSubmitBuffer for &'static [u8] {
    fn extract(self) -> (*const u8, usize) {
        (self.as_ptr(), self.len())
    }
}

impl<const T: usize> IORingSubmitBuffer for &'static [u8; T] {
    fn extract(self) -> (*const u8, usize) {
        (self.as_ptr(), T)
    }
}
