use std::arch::x86_64;

pub unsafe fn memcpy_unsafe(src: *const u8, dst: *mut u8, length: usize) {
    let mut i = 0;
    let size = 16;

    while i + size <= length {
        let source = src.add(i) as *const x86_64::__m128i;
        let destination = dst.add(i) as *mut x86_64::__m128i;

        let data = x86_64::_mm_loadu_si128(source);
        x86_64::_mm_storeu_si128(destination, data);

        i += size;
    }

    while i < length && length - i >= 4 {
        let src = src.add(i) as *const u32;
        let dst = dst.add(i) as *mut u32;

        *dst = *src;
        i += 4;
    }

    while i < length && length - i < 16 {
        *dst.add(i) = *src.add(i);
        i += 1;
    }
}

pub unsafe fn memcpy_unsafe_overlapped(src: *const u8, dst: *mut u8, length: usize) {
    let mut i = 0;

    while i < length {
        *dst.add(i) = *src.add(i);
        i += 1;
    }
}
