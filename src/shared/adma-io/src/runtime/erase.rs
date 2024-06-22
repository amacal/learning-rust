use ::core::future::Future;
use ::core::mem;
use ::core::pin::Pin;
use ::core::task::Context;
use ::core::task::Poll;

use crate::heap::*;
use crate::trace::*;

pub struct PollableTarget {
    target: *mut (),
    poll: fn(*mut (), &mut Context<'_>) -> Poll<Option<&'static [u8]>>,
}

impl PollableTarget {
    pub fn from<F>(target: *mut F) -> Self
    where
        F: Future<Output = Option<&'static [u8]>>,
    {
        fn poll<F>(target: *mut (), cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>>
        where
            F: Future<Output = Option<&'static [u8]>>,
        {
            unsafe { Pin::new_unchecked(&mut *(target as *mut F)).poll(cx) }
        }

        Self {
            target: target as *mut (),
            poll: poll::<F>,
        }
    }

    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>> {
        (self.poll)(self.target, cx)
    }
}

pub struct CallableTarget {
    target: Heap,
    call: fn(&Heap) -> Option<&'static [u8]>,
}

#[repr(C)]
struct CallableHeader {
    data: [usize; 4],
    call: fn(&Heap) -> Option<&'static [u8]>,
}

#[repr(C)]
struct CallableArgs<F, R, E>
where
    F: FnOnce() -> Result<R, E>,
{
    header: CallableHeader,
    target: Option<F>,
    result: Option<Result<R, E>>,
}

impl<F, R, E> CallableArgs<F, R, E>
where
    F: FnOnce() -> Result<R, E>,
{
    pub fn call(&mut self) -> Option<&'static [u8]> {
        self.result = match self.target.take() {
            None => return Some(b"calling callable; failed"),
            Some(target) => Some(target.call_once(())),
        };

        None
    }
}

impl<F, R, E> HeapLifetime for CallableArgs<F, R, E>
where
    F: FnOnce() -> Result<R, E>,
{
    fn ctor(&mut self) {}
    fn dtor(&mut self) {}
}

impl CallableTarget {
    fn new(target: Heap, call: fn(&Heap) -> Option<&'static [u8]>) -> Self {
        Self { target, call }
    }

    pub fn as_ptr(&self) -> (usize, usize) {
        (self.target.ptr, self.target.len)
    }

    pub fn from(heap: Heap) -> Self {
        let header: View<CallableHeader> = heap.view();
        let target: CallableTarget = CallableTarget::new(heap, header.call);

        target
    }
}

pub enum CallableTargetAllocate {
    Succeeded(CallableTarget),
    AllocationFailed(isize),
}

impl CallableTarget {
    pub fn allocate<const T: usize, F, R, E>(pool: &mut HeapPool<T>, target: F) -> CallableTargetAllocate
    where
        F: FnOnce() -> Result<R, E> + Send,
    {
        fn call<F, R, E>(target: &Heap) -> Option<&'static [u8]>
        where
            F: FnOnce() -> Result<R, E>,
        {
            let mut args: View<CallableArgs<F, R, E>> = target.view();
            let result: Option<&[u8]> = args.call();

            result
        }

        let len = mem::size_of::<CallableArgs<F, R, E>>();
        trace1(b"allocating callable; soft, size=%d\n", len);

        let heap = match pool.acquire(len) {
            Some(reference) => Heap::from(reference),
            None => {
                trace1(b"allocating callable; hard, size=%d\n", len);

                match mem_alloc(len) {
                    MemoryAllocation::Succeeded(heap) => heap,
                    MemoryAllocation::Failed(err) => return CallableTargetAllocate::AllocationFailed(err),
                }
            }
        };

        let (ptr, len) = heap.as_ptr();
        let mut data = heap.boxed::<CallableArgs<F, R, E>>();

        data.result = None;
        data.target = Some(target);
        data.header = CallableHeader {
            data: [ptr, len, 0, 0],
            call: call::<F, R, E>,
        };

        CallableTargetAllocate::Succeeded(Self {
            target: data.into(),
            call: call::<F, R, E>,
        })
    }

    pub fn release(mut self, pool: &mut HeapPool<16>) {
        trace1(b"releasing callable; soft, addr=%x\n", self.target.ptr);

        if let Some(_) = pool.release(self.target.as_ref()) {
            trace1(b"releasing callable; hard, addr=%x\n", self.target.ptr);
            mem_free(&mut self.target);
        }
    }
}

impl CallableTarget {
    pub fn call(&mut self) -> Option<&'static [u8]> {
        trace2(
            b"dispatching callable; target=%x, size=%d\n",
            self.target.ptr,
            self.target.len,
        );

        (self.call)(&mut self.target)
    }

    pub fn result<F, R, E>(self, pool: &mut HeapPool<16>) -> Option<Result<R, E>>
    where
        F: FnOnce() -> Result<R, E>,
    {
        let value = self.target.view::<CallableArgs<F, R, E>>().result.take();
        self.release(pool);
        value
    }
}
