use ::core::mem;
use ::core::task::Context;
use ::core::task::Poll;

use super::erase::*;
use super::pin::*;
use super::refs::*;
use crate::heap::*;
use crate::trace::*;

const TASKS_COUNT: usize = 128;
const COMPLETERS_COUNT: usize = 256;

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
    heap: Heap,
    completions: usize,
    target: PollableTarget,
    result: Option<&'static [u8]>,
}

impl IORingTask {
    pub fn poll(&self, cx: &mut Context<'_>) -> Poll<Option<&'static [u8]>> {
        self.target.poll(cx)
    }

    pub fn release(mut self) -> Option<&'static [u8]> {
        self.heap.free();
        return self.result.take();
    }
}

pub struct IORingRegistry {
    tasks_id: u32,
    tasks_count: usize,
    tasks_slots: [usize; TASKS_COUNT],
    tasks_array: [Option<IORingTask>; TASKS_COUNT],
    completers_id: u32,
    completers_count: usize,
    completers_slots: [usize; COMPLETERS_COUNT],
    completers_array: [Option<IORingTaskCompletion>; COMPLETERS_COUNT],
}

impl IORingRegistry {
    pub fn tasks(&self) -> usize {
        self.tasks_count
    }

    pub fn completers(&self) -> usize {
        self.completers_count
    }
}

impl HeapLifetime for IORingRegistry {
    fn ctor(&mut self) {
        self.tasks_id = 0;
        self.tasks_count = 0;

        self.completers_id = 0;
        self.completers_count = 0;

        unsafe {
            for i in 0..TASKS_COUNT {
                *self.tasks_array.get_unchecked_mut(i) = None;
                *self.tasks_slots.get_unchecked_mut(i) = i;
            }

            for i in 0..COMPLETERS_COUNT {
                *self.completers_array.get_unchecked_mut(i) = None;
                *self.completers_slots.get_unchecked_mut(i) = i;
            }
        }
    }

    fn dtor(&mut self) {}
}

pub enum IORingRegistryAllocation {
    Succeeded(Boxed<IORingRegistry>),
    AllocationFailed(isize),
}

impl IORingRegistry {
    pub fn allocate() -> IORingRegistryAllocation {
        match Heap::allocate(mem::size_of::<IORingRegistry>()) {
            Ok(heap) => IORingRegistryAllocation::Succeeded(heap.boxed()),
            Err(err) => IORingRegistryAllocation::AllocationFailed(err),
        }
    }
}

pub enum IORingRegistryAppend<T> {
    Succeeded(T),
    NotEnoughSlots(),
    InternallyFailed(),
}

impl IORingRegistry {
    pub fn append_task(&mut self, pinned: IORingPin) -> IORingRegistryAppend<IORingTaskRef> {
        let tidx = match self.tasks_slots.get(self.tasks_count) {
            Some(tidx) => {
                trace1(b"appending task to registry; tidx=%d\n", *tidx);
                *tidx
            }
            None => {
                trace0(b"appending task to registry; not enough slots\n");
                return IORingRegistryAppend::NotEnoughSlots();
            }
        };

        let (heap, target) = match pinned.components() {
            Some((heap, target)) => {
                trace2(b"appending task to registry; tidx=%d, addr=%x\n", tidx, heap.ptr());
                (heap, target)
            }
            None => {
                trace1(b"appending task to registry; tidx=%d, not enough components\n", tidx);
                return IORingRegistryAppend::InternallyFailed();
            }
        };

        self.tasks_id += 1;
        self.tasks_count += 1;

        unsafe {
            *self.tasks_array.get_unchecked_mut(tidx) = Some(IORingTask {
                tid: self.tasks_id,
                heap: heap,
                target: target,
                completions: 0,
                flags: 0,
                result: None,
            });
        }

        trace2(b"appending task to registry; tidx=%d, tid=%d\n", tidx, self.tasks_id);
        IORingRegistryAppend::Succeeded(IORingTaskRef::new(tidx, self.tasks_id))
    }

    pub fn append_completer(&mut self, task: IORingTaskRef) -> IORingRegistryAppend<IORingCompleterRef> {
        let tidx = task.tidx() % TASKS_COUNT;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        let task = match node {
            Some(task) => task,
            None => {
                trace1(b"appending completer to registry; tid=%d, task not found\n", task.tid());
                return IORingRegistryAppend::InternallyFailed();
            }
        };

        let cidx = match self.completers_slots.get(self.completers_count) {
            Some(cidx) => {
                trace2(b"appending completer to registry; tid=%d, cidx=%d\n", task.tid, *cidx);
                *cidx
            }
            None => {
                trace1(b"appending completer to registry; tid=%d, not enough slots\n", task.tid);
                return IORingRegistryAppend::NotEnoughSlots();
            }
        };

        self.completers_id += 1;
        self.completers_count += 1;
        task.completions += 1;

        trace3(
            b"appending completer to registry; tid=%d, cidx=%d, cid=%d\n",
            task.tid,
            cidx,
            self.completers_id,
        );

        unsafe {
            *self.completers_array.get_unchecked_mut(cidx) = Some(IORingTaskCompletion {
                cid: self.completers_id,
                tidx: tidx,
                flags: 0,
                result: None,
            });
        }

        IORingRegistryAppend::Succeeded(IORingCompleterRef::new(cidx, self.completers_id))
    }
}

pub enum IORingRegistryRemove<T> {
    Succeeded(T),
    NotReady(),
    NotFound(),
}

impl IORingRegistry {
    pub fn remove_task(&mut self, task: &IORingTaskRef) -> IORingRegistryRemove<IORingTask> {
        let tidx = task.tidx() % TASKS_COUNT;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        let found = match node {
            Some(found) if found.tid == task.tid() => found,
            Some(_) | None => return IORingRegistryRemove::NotFound(),
        };

        if found.flags & 0x01 != 0x01 {
            return IORingRegistryRemove::NotReady();
        }

        self.tasks_count -= 1;
        unsafe { *self.tasks_slots.get_unchecked_mut(self.tasks_count) = tidx };

        trace2(
            b"removing task; tidx=%d, tid=%d\n",
            tidx,
            found.tid,
        );

        match node.take() {
            None => IORingRegistryRemove::NotFound(),
            Some(found) => IORingRegistryRemove::Succeeded(found),
        }
    }

    pub fn remove_completer(&mut self, completer: &IORingCompleterRef) -> IORingRegistryRemove<IORingTaskCompletion> {
        let cidx = completer.cidx() % COMPLETERS_COUNT;
        let node = unsafe { self.completers_array.get_unchecked_mut(cidx) };

        let found = match node {
            Some(found) if found.cid == completer.cid() => found,
            Some(_) | None => return IORingRegistryRemove::NotFound(),
        };

        if found.flags & 0x01 != 0x01 {
            return IORingRegistryRemove::NotReady();
        }

        self.completers_count -= 1;
        unsafe { *self.completers_slots.get_unchecked_mut(self.completers_count) = cidx };

        trace2(
            b"removing completer; cidx=%d, cid=%d\n",
            cidx,
            found.cid,
        );

        match node.take() {
            None => IORingRegistryRemove::NotFound(),
            Some(found) => IORingRegistryRemove::Succeeded(found),
        }
    }
}

#[allow(dead_code)]
pub enum IORingRegistryPoll {
    Ready(usize, Option<&'static [u8]>),
    Pending(usize),
    NotFound(),
}

impl IORingRegistry {
    pub fn poll(&mut self, task: &IORingTaskRef, cx: &mut Context<'_>) -> IORingRegistryPoll {
        let tidx = task.tidx() % TASKS_COUNT;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        // task has to be guarded against tid
        let found = match node {
            Some(found) if found.tid == task.tid() => found,
            Some(_) | None => return IORingRegistryPoll::NotFound(),
        };

        // regular pending or actual future result
        let value = match found.poll(cx) {
            Poll::Pending => return IORingRegistryPoll::Pending(found.completions),
            Poll::Ready(value) => value,
        };

        // task reported as ready
        found.flags |= 0x01;
        found.result = value;

        // we return also number of remaining completions
        IORingRegistryPoll::Ready(found.completions, value)
    }
}

pub enum IORingRegistryComplete {
    Succeeded(IORingTaskRef, bool, usize),
    Inconsistent(),
    NotFound(),
}

impl IORingRegistry {
    pub fn complete(&mut self, completer: IORingCompleterRef, result: i32) -> IORingRegistryComplete {
        let cidx = completer.cidx() % COMPLETERS_COUNT;
        let node = unsafe { self.completers_array.get_unchecked_mut(cidx) };

        trace2(b"looking for completions; cidx=%d, res=%d\n", cidx, result);
        let completer = match node {
            Some(found) if found.cid == completer.cid() => found,
            Some(_) | None => {
                trace1(b"looking for completions; cidx=%d, not found\n", cidx);
                return IORingRegistryComplete::NotFound();
            }
        };

        // completer is ready
        completer.flags |= 0x01;
        completer.result = Some(result);

        let tidx = completer.tidx % TASKS_COUNT;
        let node = unsafe { self.tasks_array.get_unchecked_mut(tidx) };

        trace1(b"looking for tasks; tidx=%d\n", tidx);
        let task = match node {
            Some(task) => task,
            None => {
                trace1(b"looking for tasks; tidx=%d, not found\n", tidx);
                return IORingRegistryComplete::NotFound();
            }
        };

        trace2(b"looking for tasks; tidx=%d, tid=%d\n", tidx, task.tid);

        // task has to be updated
        if task.completions > 0 {
            task.completions -= 1;
            trace2(
                b"looking for tasks; tidx=%d, left=%d, decreased\n",
                tidx,
                task.completions,
            );
        } else {
            trace1(b"looking for tasks; tidx=%d, depth inconsistency\n", tidx);
            return IORingRegistryComplete::Inconsistent();
        }

        let is_ready = task.flags & 0x01 == 0x01;
        let completions = task.completions;

        let task = IORingTaskRef::new(completer.tidx, task.tid);
        return IORingRegistryComplete::Succeeded(task, is_ready, completions);
    }
}
