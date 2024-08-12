use ::core::future::*;
use ::core::mem;
use ::core::ops::*;
use ::core::pin::*;
use ::core::ptr;
use ::core::task::*;

use crate::heap::*;
use crate::trace::*;

pub struct PollableTarget {
    target: Heap,
    poll: fn(&Heap, &mut Context<'_>) -> Poll<Option<&'static [u8]>>,
}

impl PollableTarget {
    fn from<F>(target: Heap) -> Self
    where
        F: Future<Output = Option<&'static [u8]>>,
    {
        fn poll<F>(target: &Heap, cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>>
        where
            F: Future<Output = Option<&'static [u8]>>,
        {
            let mut view: View<F> = target.view::<F>();
            unsafe { Pin::new_unchecked(view.deref_mut()).poll(cx) }
        }

        Self {
            target: target,
            poll: poll::<F>,
        }
    }

    pub fn allocate<const T: usize, F>(_pool: &mut HeapPool<T>, target: F) -> Option<PollableTarget>
    where
        F: Future<Output = Option<&'static [u8]>>,
    {
        let size = mem::size_of::<F>() / 8 * 8 + 8;
        trace1(b"allocating memory to pin a future; size=%d\n", size);

        let heap = match Heap::allocate(size) {
            Ok(value) => {
                trace2(b"allocating memory to pin a future; size=%d, addr=%x\n", size, value.as_ref().ptr());
                value
            }
            Err(None) => {
                trace1(b"allocating memory to pin a future; size=%d, failed\n", size);
                return None;
            }
            Err(Some(errno)) => {
                trace2(b"allocating memory to pin a future; size=%d, err=%d\n", size, errno);
                return None;
            }
        };

        unsafe {
            // copy future to the heap
            let allocated = heap.as_ref();
            ptr::write(allocated.ptr() as *mut F, target);

            // and out such pointer create erasure
            Some(PollableTarget::from::<F>(heap))
        }
    }

    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>> {
        (self.poll)(&self.target, cx)
    }
}

#[cfg(test)]
mod tests {
    use core::task::Waker;

    use super::*;
    use crate::runtime::raw::*;

    #[test]
    fn allocates_pollable_once() {
        let mut pool = HeapPool::<1>::new();
        let target = async { None::<&'static [u8]> };

        let heap = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(pollable) => pollable.target.as_ref(),
        };

        assert_ne!(heap.ptr(), 0);
        assert!(heap.len() > 0);
    }

    #[test]
    fn polls_pollable() {
        let mut pool = HeapPool::<1>::new();
        let target = async { None::<&'static [u8]> };

        let raw = make_waker();
        let waker = unsafe { Waker::from_raw(raw) };
        let mut cx = Context::from_waker(&waker);

        let pollable = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        match pollable.poll(&mut cx) {
            Poll::Ready(value) => assert!(value.is_none()),
            Poll::Pending => assert!(false),
        }
    }
}
