use ::core::future::Future;
use ::core::mem;
use ::core::ptr;

use super::erase::*;
use crate::heap::*;
use crate::trace::*;

pub struct IORingPin {
    value: Option<(Heap, PollableTarget)>,
}

impl IORingPin {
    fn from(heap: Heap, target: PollableTarget) -> Self {
        Self {
            value: Some((heap, target)),
        }
    }
}

pub enum IORingPinAllocate {
    Succeeded(IORingPin),
    AllocationFailed(isize),
}

impl IORingPin {
    pub fn allocate<F>(target: F) -> IORingPinAllocate
    where
        F: Future<Output = Option<&'static [u8]>>,
    {
        let size = mem::size_of::<F>() / 8 * 8 + 8;
        trace1(b"allocating memory to pin a future; size=%d\n", size);

        let heap = match mem_alloc(size) {
            MemoryAllocation::Succeeded(value) => {
                trace2(b"allocating memory to pin a future; size=%d, addr=%x\n", size, value.ptr);
                value
            }
            MemoryAllocation::Failed(err) => {
                trace2(b"allocating memory to pin a future; size=%d,s err=%d\n", size, err);
                return IORingPinAllocate::AllocationFailed(err);
            }
        };

        unsafe {
            // copy future to the heap
            let allocated = heap.ptr as *mut F;
            ptr::write(allocated, target);

            // and out such pointer create erasure
            let erased = PollableTarget::from(allocated);
            IORingPinAllocate::Succeeded(IORingPin::from(heap, erased))
        }
    }
}

impl IORingPin {
    pub fn components(mut self) -> Option<(Heap, PollableTarget)> {
        self.value.take()
    }
}

impl Drop for IORingPin {
    fn drop(&mut self) {
        if let Some((heap, _)) = &mut self.value {
            mem_free(heap);
            self.value = None;
        }
    }
}
