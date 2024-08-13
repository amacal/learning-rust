use adma_heap::*;

use crate::core::*;

#[repr(C)]
pub struct ProcessArguments {
    argc: usize,
    argv: *const *const u8,
}

pub struct ProcessArgument {
    ptr: *const u8,
}

unsafe impl Sync for ProcessArguments {}
unsafe impl Send for ProcessArgument {}
unsafe impl Sync for ProcessArgument {}

impl ProcessArguments {
    pub fn len(&self) -> usize {
        self.argc
    }

    pub fn get(&self, index: usize) -> Option<ProcessArgument> {
        if index >= self.argc {
            return None;
        }

        let ptr = unsafe { *self.argv.add(index) as *const u8 };
        let arg = ProcessArgument { ptr: ptr };

        Some(arg)
    }

    pub fn is(&self, index: usize, value: &'static [u8]) -> bool {
        let arg = match self.get(index) {
            None => return false,
            Some(value) => value,
        };

        let mut idx = 0;
        let len = value.len();
        let value = value.as_ptr();

        unsafe {
            while idx < len {
                if *arg.ptr.add(idx) == b'\0' {
                    return false;
                }

                if *arg.ptr.add(idx) != *value.add(idx) {
                    return false;
                }

                idx += 1;
            }

            return idx == len && *arg.ptr.add(idx) == b'\0';
        }
    }

    pub fn select<const T: usize>(&self, index: usize, values: [&'static [u8]; T]) -> Option<&'static [u8]> {
        for i in 0..T {
            if self.is(index, values[i]) {
                return Some(values[i]);
            }
        }

        None
    }
}

impl AsNullTerminatedRef for ProcessArgument {
    fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
}

impl Pinned for ProcessArgument {
    fn into(self: Self) -> HeapRef {
        HeapRef::new(self.ptr as usize, 0)
    }

    fn from(heap: HeapRef) -> Self {
        Self {
            ptr: heap.ptr() as *const u8,
        }
    }
}
