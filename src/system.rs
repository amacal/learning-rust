use core::{arch::global_asm, ffi::CStr};

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
    pub fn get(&self, index: usize) -> Option<&CStr> {
        if index >= self.argc {
            return None
        }

        unsafe {
            Some(CStr::from_ptr(*self.argv.add(index) as *const i8))
        }
    }
}
