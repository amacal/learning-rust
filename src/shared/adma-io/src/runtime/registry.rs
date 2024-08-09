use ::core::task::Context;
use ::core::task::Poll;

use super::pollable::*;
use super::refs::*;
use crate::heap::*;
use crate::trace::*;

pub enum IORegistryError {
    AllocationFailed,
    NotEnoughSlots,
    TaskNotFound,
    TaskNotReady,
    CompleterNotFound,
    CompleterNotReady,
}

pub struct IORingTaskCompletion {
    cid: u32,
    tidx: usize,
    flags: u32,
    result: Option<i32>,
}

impl IORingTaskCompletion {
    pub fn result(self) -> Option<i32> {
        self.result
    }
}

pub struct IORingTask {
    tid: u32,
    flags: u32,
    completions: usize,
    target: PollableTarget,
    result: Option<&'static [u8]>,
}

impl IORingTask {
    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>> {
        self.target.poll(cx)
    }

    pub fn release(mut self) -> Option<&'static [u8]> {
        return self.result.take();
    }
}

pub struct IORingRegistry<const T: usize, const C: usize> {
    tasks_id: u32,
    tasks_count: usize,
    tasks_slots: [usize; T],
    tasks_array: [Option<IORingTask>; T],
    completers_id: u32,
    completers_count: usize,
    completers_slots: [usize; C],
    completers_array: [Option<IORingTaskCompletion>; C],
}

impl<const T: usize, const C: usize> IORingRegistry<C, T> {
    pub fn tasks(&self) -> usize {
        self.tasks_count
    }

    pub fn completers(&self) -> usize {
        self.completers_count
    }
}

impl<const T: usize, const C: usize> IORingRegistry<T, C> {
    pub fn allocate() -> Result<Self, IORegistryError> {
        let mut tasks_slots = [0; T];
        let tasks_array = [const { None }; T];

        let mut completers_slots = [0; C];
        let completers_array = [const { None }; C];

        unsafe {
            for i in 0..T {
                *tasks_slots.get_unchecked_mut(i) = i;
            }

            for i in 0..C {
                *completers_slots.get_unchecked_mut(i) = i;
            }
        }

        Ok(Self {
            tasks_id: 0,
            tasks_count: 0,
            tasks_array: tasks_array,
            tasks_slots: tasks_slots,
            completers_id: 0,
            completers_count: 0,
            completers_array: completers_array,
            completers_slots: completers_slots,
        })
    }

    fn drop_by_reference(&mut self) {
        trace2(b"releasing registry droplet; tasks=%d, completers=%d\n", T, C);
    }

    pub fn droplet(self) -> Droplet<Self> {
        trace2(b"creating registry droplet; tasks=%d, completers=%d\n", T, C);
        Droplet::from(self, Self::drop_by_reference)
    }
}

impl<const T: usize, const C: usize> IORingRegistry<T, C> {
    pub fn prepare_task(&mut self) -> Result<IORingTaskRef, IORegistryError> {
        let tidx = match self.tasks_slots.get(self.tasks_count) {
            Some(tidx) => {
                trace1(b"appending task to registry; tidx=%d\n", *tidx);
                *tidx
            }
            None => {
                trace0(b"appending task to registry; not enough slots\n");
                return Err(IORegistryError::NotEnoughSlots);
            }
        };

        self.tasks_id = self.tasks_id.wrapping_add(1);
        self.tasks_count = self.tasks_count.wrapping_add(1);

        trace2(b"appending task to registry; tidx=%d, tid=%d\n", tidx, self.tasks_id);
        Ok(IORingTaskRef::new(tidx, self.tasks_id))
    }

    pub fn append_task(&mut self, task: IORingTaskRef, target: PollableTarget) -> IORingTaskRef {
        let (tid, tidx) = (task.tid(), task.tidx());
        let task = IORingTask {
            tid: tid,
            target: target,
            completions: 0,
            flags: 0,
            result: None,
        };

        unsafe {
            *self.tasks_array.get_unchecked_mut(tidx) = Some(task);
        }

        trace2(b"appending task to registry; tidx=%d, tid=%d\n", tidx, tid);
        IORingTaskRef::new(tidx, tid)
    }

    pub fn append_completer(&mut self, task: &IORingTaskRef) -> Result<IORingCompleterRef, IORegistryError> {
        let tidx = task.tidx() % T;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        let task = match node {
            Some(task) => task,
            None => {
                trace1(b"appending completer to registry; tid=%d, task not found\n", task.tid());
                return Err(IORegistryError::TaskNotFound);
            }
        };

        let cidx = match self.completers_slots.get(self.completers_count) {
            Some(cidx) => {
                trace2(b"appending completer to registry; tid=%d, cidx=%d\n", task.tid, *cidx);
                *cidx
            }
            None => {
                trace1(b"appending completer to registry; tid=%d, not enough slots\n", task.tid);
                return Err(IORegistryError::NotEnoughSlots);
            }
        };

        self.completers_id = self.completers_id.wrapping_add(1);
        self.completers_count = self.completers_count.wrapping_add(1);
        task.completions = task.completions.wrapping_add(1);

        let completion = IORingTaskCompletion {
            cid: self.completers_id,
            tidx: tidx,
            flags: 0,
            result: None,
        };

        unsafe {
            *self.completers_array.get_unchecked_mut(cidx) = Some(completion);
        }

        trace3(b"appending completer to registry; tid=%d, cidx=%d, cid=%d\n", task.tid, cidx, self.completers_id);
        Ok(IORingCompleterRef::new(cidx, self.completers_id))
    }
}

impl<const T: usize, const C: usize> IORingRegistry<T, C> {
    pub fn remove_task(&mut self, task: &IORingTaskRef) -> Result<IORingTask, IORegistryError> {
        let tidx = task.tidx() % T;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        let found = match node {
            Some(found) if found.tid == task.tid() => found,
            Some(_) | None => return Err(IORegistryError::TaskNotFound),
        };

        if found.flags & 0x01 != 0x01 {
            return Err(IORegistryError::TaskNotReady);
        }

        if found.completions > 0 {
            return Err(IORegistryError::TaskNotReady);
        }

        self.tasks_count = self.tasks_count.wrapping_sub(1);
        unsafe { *self.tasks_slots.get_unchecked_mut(self.tasks_count) = tidx };

        trace2(b"removing task; tidx=%d, tid=%d\n", tidx, found.tid);

        match node.take() {
            None => Err(IORegistryError::TaskNotFound),
            Some(found) => Ok(found),
        }
    }

    pub fn remove_completer(
        &mut self,
        completer: &IORingCompleterRef,
    ) -> Result<IORingTaskCompletion, IORegistryError> {
        let cidx = completer.cidx() % C;
        let node = unsafe { self.completers_array.get_unchecked_mut(cidx) };

        let found = match node {
            Some(found) if found.cid == completer.cid() => found,
            Some(_) | None => return Err(IORegistryError::CompleterNotFound),
        };

        if found.flags & 0x01 != 0x01 {
            return Err(IORegistryError::CompleterNotReady);
        }

        self.completers_count = self.completers_count.wrapping_sub(1);
        unsafe { *self.completers_slots.get_unchecked_mut(self.completers_count) = cidx };

        trace2(b"removing completer; cidx=%d, cid=%d\n", cidx, found.cid);

        match node.take() {
            None => Err(IORegistryError::CompleterNotFound),
            Some(found) => Ok(found),
        }
    }
}

impl<const T: usize, const C: usize> IORingRegistry<T, C> {
    pub fn poll(
        &mut self,
        task: &IORingTaskRef,
        cx: &mut Context<'_>,
    ) -> Result<(usize, Poll<Option<&'static [u8]>>), IORegistryError> {
        let tidx = task.tidx() % T;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        // task has to be guarded against tid
        let found = match node {
            Some(found) if found.tid == task.tid() => found,
            Some(_) | None => return Err(IORegistryError::TaskNotFound),
        };

        // regular pending or actual future result
        let value = match found.poll(cx) {
            Poll::Pending => return Ok((found.completions, Poll::Pending)),
            Poll::Ready(value) => value,
        };

        // task reported as ready
        found.flags |= 0x01;
        found.result = value;

        // we return also number of remaining completions
        Ok((found.completions, Poll::Ready(value)))
    }
}

impl<const T: usize, const C: usize> IORingRegistry<T, C> {
    pub fn complete(
        &mut self,
        completer: &IORingCompleterRef,
        result: i32,
    ) -> Result<(IORingTaskRef, bool, usize), IORegistryError> {
        let cidx = completer.cidx() % C;
        let node = unsafe { self.completers_array.get_unchecked_mut(cidx) };

        trace2(b"looking for completions; cidx=%d, res=%d\n", cidx, result);
        let completer = match node {
            Some(found) if found.cid == completer.cid() => found,
            Some(_) | None => {
                trace1(b"looking for completions; cidx=%d, not found\n", cidx);
                return Err(IORegistryError::CompleterNotFound);
            }
        };

        // completer is ready
        completer.flags |= 0x01;
        completer.result = Some(result);

        let tidx = completer.tidx % T;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        trace1(b"looking for tasks; tidx=%d\n", tidx);
        let task = match node {
            Some(task) => task,
            None => {
                trace1(b"looking for tasks; tidx=%d, not found\n", tidx);
                return Err(IORegistryError::TaskNotFound);
            }
        };

        task.completions = task.completions.wrapping_sub(1);

        trace2(b"looking for tasks; tidx=%d, tid=%d\n", tidx, task.tid);
        trace2(b"looking for tasks; tidx=%d, left=%d, decreased\n", tidx, task.completions);

        let is_ready = task.flags & 0x01 == 0x01;
        let completions = task.completions;

        let task = IORingTaskRef::new(completer.tidx, task.tid);
        return Ok((task, is_ready, completions));
    }
}

#[cfg(test)]
mod tests {
    use ::core::ptr;
    use ::core::task::Waker;

    use super::*;
    use crate::runtime::raw::*;

    #[test]
    fn allocates_registry() {
        let registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        assert_eq!(registry.completers_count, 0);
        assert_eq!(registry.completers_id, 0);
        assert_eq!(registry.tasks_count, 0);
        assert_eq!(registry.tasks_id, 0);

        drop(registry);
    }

    #[test]
    fn appends_task_once() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        assert_eq!(task.tidx(), 0);
        assert_eq!(task.tid(), 1);

        assert_eq!(registry.completers_count, 0);
        assert_eq!(registry.completers_id, 0);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.tasks_array[0].is_some());
        drop(registry);
    }

    #[test]
    fn appends_completer_once() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        let completer = match registry.append_completer(&task) {
            Err(_) => return assert!(false),
            Ok(completer) => completer,
        };

        assert_eq!(completer.cidx(), 0);
        assert_eq!(completer.cid(), 1);

        assert_eq!(registry.completers_count, 1);
        assert_eq!(registry.completers_id, 1);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.completers_array[0].is_some());
        drop(registry);
    }

    #[test]
    fn polls_task() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        let raw = make_waker(ptr::null());
        let waker = unsafe { Waker::from_raw(raw) };
        let mut cx = Context::from_waker(&waker);

        let (cnt, val) = match registry.poll(&task, &mut cx) {
            Ok((cnt, Poll::Ready(val))) => (cnt, val),
            Ok(_) | Err(_) => return assert!(false),
        };

        assert_eq!(cnt, 0);
        assert!(val.is_none());

        assert!(registry.tasks_array[0].is_some());
        drop(registry);
    }

    #[test]
    fn removes_task_if_present_completed_polled() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        let raw = make_waker(ptr::null());
        let waker = unsafe { Waker::from_raw(raw) };
        let mut cx = Context::from_waker(&waker);

        match registry.poll(&task, &mut cx) {
            Ok((_, Poll::Ready(_))) => (),
            Ok(_) | Err(_) => assert!(false),
        };

        let tid = task.tid();
        let task = match registry.remove_task(&task) {
            Err(_) => return assert!(false),
            Ok(task) => task,
        };

        assert_eq!(task.completions, 0);
        assert_eq!(task.tid, tid);

        assert_eq!(registry.completers_count, 0);
        assert_eq!(registry.completers_id, 0);
        assert_eq!(registry.tasks_count, 0);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.tasks_array[0].is_none());
        drop(registry);
    }

    #[test]
    fn removes_task_if_present_completed_not_polled() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        match registry.remove_task(&task) {
            Err(IORegistryError::TaskNotReady) => assert!(true),
            Ok(_) | Err(_) => assert!(false),
        }

        assert_eq!(registry.completers_count, 0);
        assert_eq!(registry.completers_id, 0);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.tasks_array[0].is_some());
        drop(registry);
    }

    #[test]
    fn removes_task_if_present_completed_polled_but_with_awaiting_completer() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        match registry.append_completer(&task) {
            Err(_) => return assert!(false),
            Ok(_) => (),
        }

        let raw = make_waker(ptr::null());
        let waker = unsafe { Waker::from_raw(raw) };
        let mut cx = Context::from_waker(&waker);

        match registry.poll(&task, &mut cx) {
            Ok((_, Poll::Ready(_))) => (),
            Ok(_) | Err(_) => assert!(false),
        };

        match registry.remove_task(&task) {
            Err(IORegistryError::TaskNotReady) => assert!(true),
            Ok(_) | Err(_) => assert!(false),
        }

        assert_eq!(registry.completers_count, 1);
        assert_eq!(registry.completers_id, 1);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.tasks_array[0].is_some());
        assert!(registry.completers_array[0].is_some());

        drop(registry);
    }

    #[test]
    fn removes_task_if_not_present() {
        let task = IORingTaskRef::new(2, 1);
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        match registry.remove_task(&task) {
            Err(IORegistryError::TaskNotFound) => assert!(true),
            Ok(_) | Err(_) => assert!(false),
        }

        assert_eq!(registry.completers_count, 0);
        assert_eq!(registry.completers_id, 0);
        assert_eq!(registry.tasks_count, 0);
        assert_eq!(registry.tasks_id, 0);

        assert!(registry.tasks_array[0].is_none());
        drop(registry);
    }

    #[test]
    fn removes_completer_if_present_completed() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        let completer = match registry.append_completer(&task) {
            Err(_) => return assert!(false),
            Ok(completer) => completer,
        };

        match registry.complete(&completer, 13) {
            Ok(_) => assert!(true),
            _ => return assert!(false),
        }

        let cid = completer.cid();
        let completion = match registry.remove_completer(&completer) {
            Err(_) => return assert!(false),
            Ok(task) => task,
        };

        assert_eq!(completion.cid, cid);
        assert_eq!(completion.tidx, task.tidx());

        match completion.result() {
            None => return assert!(false),
            Some(result) => assert_eq!(result, 13),
        }

        assert_eq!(registry.completers_count, 0);
        assert_eq!(registry.completers_id, 1);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.completers_array[0].is_none());
        assert!(registry.tasks_array[0].is_some());

        drop(registry);
    }

    #[test]
    fn removes_completer_if_present_not_completed() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        let completer = match registry.append_completer(&task) {
            Err(_) => return assert!(false),
            Ok(completer) => completer,
        };

        match registry.remove_completer(&completer) {
            Err(IORegistryError::CompleterNotReady) => assert!(true),
            Ok(_) | Err(_) => assert!(false),
        }

        assert_eq!(registry.completers_count, 1);
        assert_eq!(registry.completers_id, 1);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.completers_array[0].is_some());
        assert!(registry.tasks_array[0].is_some());

        drop(registry);
    }

    #[test]
    fn removes_completer_if_present_not_found() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        match registry.append_completer(&task) {
            Err(_) => assert!(false),
            Ok(_) => assert!(true),
        }

        let completer = IORingCompleterRef::new(13, 17);
        match registry.remove_completer(&completer) {
            Err(IORegistryError::CompleterNotFound) => assert!(true),
            Ok(_) | Err(_) => assert!(false),
        }

        assert_eq!(registry.completers_count, 1);
        assert_eq!(registry.completers_id, 1);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.completers_array[0].is_some());
        assert!(registry.tasks_array[0].is_some());

        drop(registry);
    }

    #[test]
    fn completes_if_both_task_and_completer_present() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        let completer = match registry.append_completer(&task) {
            Err(_) => return assert!(false),
            Ok(completer) => completer,
        };

        match registry.complete(&completer, 13) {
            Ok((_, _, _)) => assert!(true),
            _ => return assert!(false),
        }

        let cid = completer.cid();
        let completion = match registry.remove_completer(&completer) {
            Err(_) => return assert!(false),
            Ok(task) => task,
        };

        assert_eq!(completion.cid, cid);
        assert_eq!(completion.tidx, task.tidx());

        match completion.result() {
            None => return assert!(false),
            Some(result) => assert_eq!(result, 13),
        }

        assert_eq!(registry.completers_count, 0);
        assert_eq!(registry.completers_id, 1);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.completers_array[0].is_none());
        assert!(registry.tasks_array[0].is_some());

        drop(registry);
    }

    #[test]
    fn completes_if_task_present_but_completer_not() {
        let mut pool = HeapPool::<1>::new();
        let mut registry = match IORingRegistry::<2, 4>::allocate() {
            Ok(registry) => registry,
            _ => return assert!(false),
        };

        let target = async { None::<&'static [u8]> };
        let target = match PollableTarget::allocate(&mut pool, target) {
            None => return assert!(false),
            Some(target) => target,
        };

        let task = match registry.prepare_task() {
            Err(_) => return assert!(false),
            Ok(task) => registry.append_task(task, target),
        };

        match registry.append_completer(&task) {
            Err(_) => return assert!(false),
            Ok(_) => (),
        }

        let completer = IORingCompleterRef::new(13, 17);
        match registry.complete(&completer, 23) {
            Err(IORegistryError::CompleterNotFound) => assert!(true),
            Ok(_) | Err(_) => assert!(false),
        }

        assert_eq!(registry.completers_count, 1);
        assert_eq!(registry.completers_id, 1);
        assert_eq!(registry.tasks_count, 1);
        assert_eq!(registry.tasks_id, 1);

        assert!(registry.completers_array[0].is_some());
        assert!(registry.tasks_array[0].is_some());

        drop(registry);
    }
}
