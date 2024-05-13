use core::arch::global_asm;

global_asm! {
    ".global _start",
    "_start:",
    "mov rdi, [rsp]",
    "lea rsi, [rsp + 8]",
    "push rsi",
    "push rdi",
    "mov rdi, rsp",
    "call main"
}

#[repr(C)]
pub struct ProcessArguments {
    argc: usize,
    argv: *const *const u8,
}

impl ProcessArguments {
    pub fn get(&self, index: usize) -> Option<*const u8> {
        if index >= self.argc {
            return None
        }

        unsafe { Some(*self.argv.add(index) as *const u8) }
    }

    pub fn is(&self, index: usize, value: &'static [u8]) -> bool {
        let arg = match self.get(index) {
            None => return false,
            Some(value) => value
        };

        let mut idx = 0;
        let len = value.len();
        let value = value.as_ptr();

        unsafe {
            while idx < len {
                if *arg.add(idx) == b'\0' {
                    return false;
                }

                if *arg.add(idx) != *value.add(idx) {
                    return false;
                }

                idx += 1;
            }

            return idx == len && *arg.add(idx) == b'\0';
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
