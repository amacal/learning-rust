use ::core::mem;

use crate::heap::*;
use crate::trace::*;

pub enum CallableError {
    ReleaseFailed,
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

    pub fn as_ref(&self) -> HeapRef {
        self.target.as_ref()
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

                match Heap::allocate(len) {
                    Ok(heap) => heap,
                    Err(err) => return CallableTargetAllocate::AllocationFailed(err),
                }
            }
        };

        let (ptr, len) = heap.as_ref().as_ptr();
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

    pub fn release<const T: usize>(self, pool: &mut HeapPool<T>) -> Result<(), CallableError> {
        let ptr = self.target.as_ref().ptr();
        trace1(b"releasing callable; soft, addr=%x\n", ptr);

        if let Some(_) = pool.release(self.target.as_ref()) {
            trace1(b"releasing callable; hard, addr=%x\n", ptr);

            if self.target.free().is_err() {
                return Err(CallableError::ReleaseFailed)
            }
        }

        Ok(())
    }
}

impl CallableTarget {
    pub fn call(&mut self) -> Option<&'static [u8]> {
        trace2(b"dispatching callable; target=%x, size=%d\n", self.target.as_ref().ptr(), self.target.as_ref().len());

        (self.call)(&mut self.target)
    }

    pub fn result<const T: usize, F, R, E>(self, pool: &mut HeapPool<T>) -> Result<Option<Result<R, E>>, CallableError>
    where
        F: FnOnce() -> Result<R, E>,
    {
        let mut view: View<CallableArgs<F, R, E>> = self.target.view();
        let value: Option<Result<R, E>> = view.result.take();

        // when target cannot be released we propagate it down
        if let Err(err) = self.release(pool) {
            return Err(err);
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_callable_once() {
        let mut pool = HeapPool::<16>::new();
        let target = || -> Result<(), ()> { Ok(()) };

        let heap = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::Succeeded(val) => val.as_ref(),
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
        };

        assert_ne!(heap.ptr(), 0);
        assert!(heap.len() > 0);
    }

    #[test]
    fn allocates_callable_twice() {
        let mut pool = HeapPool::<1>::new();
        let target = || -> Result<(), ()> { Ok(()) };

        let first = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            CallableTargetAllocate::Succeeded(val) => {
                assert_ne!(val.as_ref().ptr(), 0);
                assert!(val.as_ref().len() > 0);

                val.as_ref().as_ptr()
            }
        };

        let second = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            CallableTargetAllocate::Succeeded(val) => {
                assert_ne!(val.as_ref().ptr(), 0);
                assert!(val.as_ref().len() > 0);

                val.as_ref().as_ptr()
            }
        };

        assert_ne!(first, second);
    }

    #[test]
    fn allocates_callable_twice_with_release() {
        let mut pool = HeapPool::<1>::new();
        let target = || -> Result<(), ()> { Ok(()) };

        let first = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            CallableTargetAllocate::Succeeded(val) => {
                assert_ne!(val.as_ref().ptr(), 0);
                assert!(val.as_ref().len() > 0);

                let pair = val.as_ref().as_ptr();
                assert!(val.release(&mut pool).is_ok());

                pair
            }
        };

        let second = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            CallableTargetAllocate::Succeeded(val) => {
                assert_ne!(val.as_ref().ptr(), 0);
                assert!(val.as_ref().len() > 0);

                val.as_ref().as_ptr()
            }
        };

        assert_eq!(first, second);
    }

    #[test]
    fn releases_callable() {
        let mut pool = HeapPool::<16>::new();
        let target = || -> Result<(), ()> { Ok(()) };

        let callable = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::Succeeded(val) => val,
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
        };

        assert!(callable.release(&mut pool).is_ok());
    }

    #[test]
    fn calls_callable_once() {
        let mut pool = HeapPool::<16>::new();
        let target = || -> Result<(), ()> { Ok(()) };

        let mut callable = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::Succeeded(val) => val,
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
        };

        match callable.call() {
            Some(_) => assert!(false),
            None => assert!(true),
        }
    }

    #[test]
    fn calls_callable_twice() {
        let mut pool = HeapPool::<16>::new();
        let target = || -> Result<(), ()> { Ok(()) };

        let mut callable = match CallableTarget::allocate(&mut pool, target) {
            CallableTargetAllocate::Succeeded(val) => val,
            CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
        };

        match callable.call() {
            Some(_) => assert!(false),
            None => assert!(true),
        }

        match callable.call() {
            Some(_) => assert!(true),
            None => assert!(false),
        }
    }

    #[test]
    fn results_callable_value() {
        let mut pool = HeapPool::<16>::new();
        let target = || -> Result<u8, ()> { Ok(13) };

        fn execute<F>(pool: &mut HeapPool<16>, target: F)
        where
            F: FnOnce() -> Result<u8, ()> + Send,
        {
            let mut callable = match CallableTarget::allocate(pool, target) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            match callable.call() {
                Some(_) => assert!(false),
                None => assert!(true),
            }

            match callable.result::<16, F, u8, ()>(pool) {
                Ok(Some(Ok(val))) => assert_eq!(val, 13),
                _ => assert!(false),
            }
        }

        execute(&mut pool, target);
    }

    #[test]
    fn results_callable_error() {
        let mut pool = HeapPool::<16>::new();
        let target = || -> Result<(), u8> { Err(13) };

        fn execute<F>(pool: &mut HeapPool<16>, target: F)
        where
            F: FnOnce() -> Result<(), u8> + Send,
        {
            let mut callable = match CallableTarget::allocate(pool, target) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            match callable.call() {
                Some(_) => assert!(false),
                None => assert!(true),
            }

            match callable.result::<16, F, (), u8>(pool) {
                Ok(Some(Err(err))) => assert_eq!(err, 13),
                _ => assert!(false),
            }
        }

        execute(&mut pool, target);
    }

    #[test]
    fn fails_callable_if_not_called() {
        let mut pool = HeapPool::<16>::new();
        let target = || -> Result<u8, ()> { Ok(13) };

        fn execute<F>(pool: &mut HeapPool<16>, target: F)
        where
            F: FnOnce() -> Result<u8, ()> + Send,
        {
            let callable = match CallableTarget::allocate(pool, target) {
                CallableTargetAllocate::Succeeded(val) => val,
                CallableTargetAllocate::AllocationFailed(_) => return assert!(false),
            };

            match callable.result::<16, F, u8, ()>(pool) {
                Ok(None) => assert!(true),
                _ => assert!(false),
            }
        }

        execute(&mut pool, target);
    }
}
