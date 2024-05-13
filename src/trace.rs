use core::ptr::null;

#[allow(dead_code)]
pub trait FormatArg {
    fn to_number(self) -> Option<impl FormatNumber>;
    fn to_string(self) -> Option<impl FormatString>;
}

pub trait FormatNumber {
    fn is_zero(&self) -> bool;
    fn is_negative(&self) -> bool;

    fn absolute(&self) -> Self;
    fn divide<const T: u8>(&self) -> (Self, u8)
    where
        Self: Sized;
}

pub trait FormatString {
    fn ptr(&self) -> *const u8;
    fn len(&self) -> usize;
}

#[derive(Clone, Copy)]
struct Nope {}

impl FormatString for Nope {
    fn ptr(&self) -> *const u8 {
        null()
    }

    fn len(&self) -> usize {
        0
    }
}

impl FormatNumber for Nope {
    fn is_zero(&self) -> bool {
        true
    }

    fn is_negative(&self) -> bool {
        false
    }

    fn absolute(&self) -> Self {
        *self
    }

    fn divide<const T: u8>(&self) -> (Self, u8)
    where
        Self: Sized,
    {
        (*self, 0)
    }
}

impl FormatArg for &'static [u8] {
    fn to_number(self) -> Option<impl FormatNumber> {
        None::<Nope>
    }

    fn to_string(self) -> Option<impl FormatString> {
        Some(self)
    }
}

impl FormatString for &'static [u8] {
    fn ptr(&self) -> *const u8 {
        self.as_ptr()
    }

    fn len(&self) -> usize {
        <[u8]>::len(self)
    }
}

impl<T> FormatArg for *const T {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(((self as usize) & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl<T> FormatArg for *mut T {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(((self as usize) & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatArg for u32 {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(self)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatNumber for u32 {
    fn is_zero(&self) -> bool {
        *self == 0
    }

    fn is_negative(&self) -> bool {
        false
    }

    fn absolute(&self) -> Self {
        *self
    }

    fn divide<const T: u8>(&self) -> (Self, u8) {
        (*self / (T as u32), (*self % (T as u32)) as u8)
    }
}

impl FormatArg for i32 {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(self)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatNumber for i32 {
    fn is_zero(&self) -> bool {
        *self == 0
    }

    fn is_negative(&self) -> bool {
        *self < 0
    }

    fn absolute(&self) -> Self {
        if *self < 0 {
            -(*self)
        } else {
            *self
        }
    }

    fn divide<const T: u8>(&self) -> (Self, u8) {
        (*self / (T as i32), (*self % (T as i32)) as u8)
    }
}

impl FormatArg for u64 {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some((self & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatArg for usize {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some((self & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatArg for isize {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some((self & 0xffffffff) as i32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

#[allow(dead_code)]
#[inline(never)]
fn format_string<T: FormatString>(msg: *mut u8, mut idx: usize, max: usize, val: T) -> usize {
    let len = val.len();
    let val = val.ptr();
    let mut off = 0;

    unsafe {
        while off < len && idx < max && *val.add(off) != b'\0' {
            *msg.add(idx) = *val.add(off);
            off += 1;
            idx += 1;
        }
    }

    idx
}

#[allow(dead_code)]
#[inline(never)]
fn format_number<T: FormatNumber, const B: u8>(msg: *mut u8, mut idx: usize, max: usize, val: T) -> usize {
    let neg = val.is_negative();
    let zero = val.is_zero();
    let mut val = val.absolute();

    let mut buf = [0; 10];
    let buf = buf.as_mut_ptr();
    let mut buf_idx = 0;

    while buf_idx < 10 && idx < max && !val.is_zero() {
        let (next, remainder) = val.divide::<B>();
        let character = match remainder {
            value if value <= 9 => b'0' + value,
            value => b'a' + value - 10,
        };

        unsafe {
            *buf.add(buf_idx) = character;
            buf_idx += 1;
        }

        val = next;
    }

    unsafe {
        if val.is_zero() {
            if neg {
                *msg.add(idx) = b'-';
                idx += 1;
            }

            if zero {
                *msg.add(idx) = b'0';
                idx += 1;
            }

            for i in (0..buf_idx).rev() {
                *msg.add(idx) = *buf.add(i);
                idx += 1;
            }
        } else {
            *msg.add(idx) = b'?';
            idx += 1;
        }
    }

    idx
}

#[allow(dead_code)]
#[cfg(not(feature = "tracing"))]
pub fn trace0(_fmt: &'static [u8]) {}

#[allow(dead_code)]
#[inline(never)]
#[cfg(feature = "tracing")]
pub fn trace0(fmt: &'static [u8]) {
    use crate::syscall::*;
    sys_write(2, fmt.as_ptr(), fmt.len());
}

#[allow(dead_code)]
#[cfg(not(feature = "tracing"))]
pub fn trace1<T1: FormatArg + Copy>(_fmt: &'static [u8], _val1: T1) {}

#[allow(dead_code)]
#[inline(never)]
#[cfg(feature = "tracing")]
pub fn trace1<T1: FormatArg + Copy>(fmt: &'static [u8], val1: T1) {
    use crate::syscall::*;

    let len = fmt.len();
    let fmt = fmt.as_ptr();

    let mut spec = false;
    let mut msg: [u8; 80] = [0; 80];

    let max = msg.len();
    let msg = msg.as_mut_ptr();

    let mut msg_idx = 0;
    let mut val_idx = 0;

    unsafe {
        for i in 0..len {
            if spec == false && *fmt.add(i) == b'%' {
                spec = true;
            } else if spec {
                spec = false;
                (val_idx, msg_idx) = match (val_idx, *fmt.add(i)) {
                    (0, b'd') => match val1.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 10>(msg, msg_idx, max, val)),
                    },
                    (0, b'x') => match val1.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 16>(msg, msg_idx, max, val)),
                    },
                    (0, b's') => match val1.to_string() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_string(msg, msg_idx, max, val)),
                    },
                    _ => (val_idx + 1, msg_idx),
                }
            } else {
                *msg.add(msg_idx) = *fmt.add(i);
                msg_idx += 1;
            }

            if msg_idx == max {
                break;
            }
        }
    }

    sys_write(2, msg, msg_idx);
}

#[allow(dead_code)]
#[cfg(not(feature = "tracing"))]
pub fn trace2<T1: FormatArg + Copy, T2: FormatArg + Copy>(_fmt: &'static [u8], _val1: T1, _val2: T2) {}

#[allow(dead_code)]
#[inline(never)]
#[cfg(feature = "tracing")]
pub fn trace2<T1: FormatArg + Copy, T2: FormatArg + Copy>(fmt: &'static [u8], val1: T1, val2: T2) {
    use crate::syscall::*;

    let len = fmt.len();
    let fmt = fmt.as_ptr();

    let mut spec = false;
    let mut msg: [u8; 80] = [0; 80];

    let max = msg.len();
    let msg = msg.as_mut_ptr();

    let mut msg_idx = 0;
    let mut val_idx = 0;

    unsafe {
        for i in 0..len {
            if spec == false && *fmt.add(i) == b'%' {
                spec = true;
            } else if spec {
                spec = false;
                (val_idx, msg_idx) = match (val_idx, *fmt.add(i)) {
                    (0, b'd') => match val1.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 10>(msg, msg_idx, max, val)),
                    },
                    (0, b'x') => match val1.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 16>(msg, msg_idx, max, val)),
                    },
                    (0, b's') => match val1.to_string() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_string(msg, msg_idx, max, val)),
                    },
                    (1, b'd') => match val2.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 10>(msg, msg_idx, max, val)),
                    },
                    (1, b'x') => match val2.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 16>(msg, msg_idx, max, val)),
                    },
                    (1, b's') => match val2.to_string() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_string(msg, msg_idx, max, val)),
                    },
                    _ => (val_idx + 1, msg_idx),
                }
            } else {
                *msg.add(msg_idx) = *fmt.add(i);
                msg_idx += 1;
            }

            if msg_idx == max {
                break;
            }
        }
    }

    sys_write(2, msg, msg_idx);
}

#[allow(dead_code)]
#[cfg(not(feature = "tracing"))]
pub fn trace3<T1, T2, T3>(_fmt: &'static [u8], _val1: T1, _val2: T2, _val3: T3)
where
    T1: FormatArg + Copy,
    T2: FormatArg + Copy,
    T3: FormatArg + Copy,
{
}

#[allow(dead_code)]
#[inline(never)]
#[cfg(feature = "tracing")]
pub fn trace3<T1, T2, T3>(fmt: &'static [u8], val1: T1, val2: T2, val3: T3)
where
    T1: FormatArg + Copy,
    T2: FormatArg + Copy,
    T3: FormatArg + Copy,
{
    use crate::syscall::*;

    let len = fmt.len();
    let fmt = fmt.as_ptr();

    let mut spec = false;
    let mut msg: [u8; 80] = [0; 80];

    let max = msg.len();
    let msg = msg.as_mut_ptr();

    let mut msg_idx = 0;
    let mut val_idx = 0;

    unsafe {
        for i in 0..len {
            if spec == false && *fmt.add(i) == b'%' {
                spec = true;
            } else if spec {
                spec = false;
                (val_idx, msg_idx) = match (val_idx, *fmt.add(i)) {
                    (0, b'd') => match val1.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 10>(msg, msg_idx, max, val)),
                    },
                    (0, b'x') => match val1.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 16>(msg, msg_idx, max, val)),
                    },
                    (0, b's') => match val1.to_string() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_string(msg, msg_idx, max, val)),
                    },
                    (1, b'd') => match val2.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 10>(msg, msg_idx, max, val)),
                    },
                    (1, b'x') => match val2.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 16>(msg, msg_idx, max, val)),
                    },
                    (1, b's') => match val2.to_string() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_string(msg, msg_idx, max, val)),
                    },
                    (2, b'd') => match val3.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 10>(msg, msg_idx, max, val)),
                    },
                    (2, b'x') => match val3.to_number() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_number::<_, 16>(msg, msg_idx, max, val)),
                    },
                    (2, b's') => match val3.to_string() {
                        None => (val_idx + 1, msg_idx),
                        Some(val) => (val_idx + 1, format_string(msg, msg_idx, max, val)),
                    },
                    _ => (val_idx + 1, msg_idx),
                }
            } else {
                *msg.add(msg_idx) = *fmt.add(i);
                msg_idx += 1;
            }

            if msg_idx == max {
                break;
            }
        }
    }

    sys_write(2, msg, msg_idx);
}
