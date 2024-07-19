use crate::heap::*;

pub trait IORingSubmitBuffer {
    fn extract(&self) -> (*const u8, usize);
}

impl IORingSubmitBuffer for Droplet<Heap> {
    fn extract(&self) -> (*const u8, usize) {
        (self.as_ref().ptr() as *const u8, self.as_ref().len())
    }
}

impl IORingSubmitBuffer for &Droplet<Heap> {
    fn extract(&self) -> (*const u8, usize) {
        (self.as_ref().ptr() as *const u8, self.as_ref().len())
    }
}

impl<'a> IORingSubmitBuffer for HeapSlice<'a> {
    fn extract(&self) -> (*const u8, usize) {
        (self.ptr() as *const u8, self.len())
    }
}

impl<'a> IORingSubmitBuffer for &HeapSlice<'a> {
    fn extract(&self) -> (*const u8, usize) {
        (self.ptr() as *const u8, self.len())
    }
}

impl IORingSubmitBuffer for &'static [u8] {
    fn extract(&self) -> (*const u8, usize) {
        (self.as_ptr(), self.len())
    }
}

impl<const T: usize> IORingSubmitBuffer for &'static [u8; T] {
    fn extract(&self) -> (*const u8, usize) {
        (self.as_ptr(), T)
    }
}

impl<const T: usize> IORingSubmitBuffer for [u8; T] {
    fn extract(&self) -> (*const u8, usize) {
        (self.as_ptr(), T)
    }
}

impl<T: IORingSubmitBuffer> IORingSubmitBuffer for (T, usize) {
    fn extract(&self) -> (*const u8, usize) {
        (self.0.extract().0, self.1)
    }
}
