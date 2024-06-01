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

pub struct CallableTarget<const T: usize> {
    target: Heap,
    call: fn(&mut Heap) -> Option<&'static [u8]>,
}

pub struct CallableHeader {
    call: fn(&mut Heap) -> Option<&'static [u8]>,
}

#[repr(C)]
pub struct CallableArgs<const T: usize, F, R>
where
    F: FnOnce() -> Result<R, Option<&'static [u8]>>,
{
    call: fn(&mut Heap) -> Option<&'static [u8]>,
    target: Option<F>,
    result: Option<R>,
}

impl<const T: usize, F, R> CallableArgs<T, F, R>
where
    F: FnOnce() -> Result<R, Option<&'static [u8]>>,
{
    pub fn call(&mut self) -> Option<&'static [u8]> {
        let result = match self.target.take() {
            None => return Some(b"Cannot call function"),
            Some(target) => target.call_once(()),
        };

        match result {
            Ok(value) => self.result = Some(value),
            Err(err) => return err,
        }

        None
    }
}

impl<const T: usize, F, R> HeapLifetime for CallableArgs<T, F, R>
where
    F: FnOnce() -> Result<R, Option<&'static [u8]>>,
{
    fn ctor(&mut self) {}
    fn dtor(&mut self) {}
}

impl<const T: usize> CallableTarget<T> {
    fn new(target: Heap, call: fn(&mut Heap) -> Option<&'static [u8]>) -> Self {
        Self { target, call }
    }

    pub fn heap(&self) -> &Heap {
        &self.target
    }

    pub fn from(heap: Heap) -> Self {
        let header: View<CallableHeader> = heap.view_at(T);
        let target: CallableTarget<T> = CallableTarget::new(heap, header.call);

        target
    }
}

pub enum CallableTargetAllocate<const T: usize> {
    Succeeded(CallableTarget<T>),
    AllocationFailed(isize),
}

impl<const T: usize> CallableTarget<T> {
    pub fn allocate<F, R>(target: F) -> CallableTargetAllocate<T>
    where
        F: FnOnce() -> Result<R, Option<&'static [u8]>>,
    {
        fn call<const T: usize, F, R>(target: &mut Heap) -> Option<&'static [u8]>
        where
            F: FnOnce() -> Result<R, Option<&'static [u8]>>,
        {
            let mut args: View<CallableArgs<T, F, R>> = target.view_at(T);
            let result: Option<&[u8]> = args.call();

            result
        }

        let len = T + mem::size_of::<CallableArgs<T, F, R>>();
        trace1(b"allocating callable; size=%d\n", len);

        let mut data = match mem_alloc(len) {
            MemoryAllocation::Succeeded(heap) => heap.boxed_at::<CallableArgs<T, F, R>>(T),
            MemoryAllocation::Failed(err) => return CallableTargetAllocate::AllocationFailed(err),
        };

        data.result = None;
        data.target = Some(target);
        data.call = call::<T, F, R>;

        CallableTargetAllocate::Succeeded(Self {
            target: data.into(),
            call: call::<T, F, R>,
        })
    }

    pub fn release(mut self) {
        trace2(b"releasing callable; addr=%x, size=%d\n", self.target.ptr, T);
        mem_free(&mut self.target);
    }
}

impl<const T: usize> CallableTarget<T> {
    pub fn call(&mut self) -> Option<&'static [u8]> {
        trace3(
            b"dispatching callable; target=%x, size=%d, offset=%d\n",
            self.target.ptr,
            self.target.len,
            T,
        );

        (self.call)(&mut self.target)
    }

    pub fn result<F, R>(self) -> Option<R>
    where
        F: FnOnce() -> Result<R, Option<&'static [u8]>>,
    {
        let value = self.target.view_at::<CallableArgs<T, F, R>>(T).result.take();
        self.release();
        value
    }
}
