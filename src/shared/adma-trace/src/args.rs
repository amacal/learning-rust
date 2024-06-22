use ::core::ptr;

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
        ptr::null()
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

impl FormatString for *const u8 {
    fn ptr(&self) -> *const u8 {
        *self
    }

    fn len(&self) -> usize {
        let mut usize = 0;

        unsafe {
            while *(*self).add(usize) != b'\0' {
                usize += 1;
            }
        }

        usize
    }
}

impl<T> FormatArg for &T {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(((self as *const T as usize) & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatArg for *const u8 {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(((self as usize) & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        Some(self)
    }
}

impl FormatArg for *const () {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(((self as usize) & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatArg for *mut () {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(((self as usize) & 0xffffffff) as u32)
    }

    fn to_string(self) -> Option<impl FormatString> {
        None::<Nope>
    }
}

impl FormatArg for u8 {
    fn to_number(self) -> Option<impl FormatNumber> {
        Some(self as u32)
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
pub fn format_string<T: FormatString>(msg: *mut u8, mut idx: usize, max: usize, val: T) -> usize {
    let len = val.len();
    let val = val.ptr();
    let mut off = 0;

    unsafe {
        while off < len && idx < max && *val.add(off) != 0 {
            *msg.add(idx) = *val.add(off);
            off += 1;
            idx += 1;
        }
    }

    idx
}

#[allow(dead_code)]
#[inline(never)]
pub fn format_number<T: FormatNumber, const B: u8, const L: usize>(
    msg: *mut u8,
    mut idx: usize,
    max: usize,
    val: T,
) -> usize {
    let neg = val.is_negative();
    let zero = val.is_zero();
    let mut val = val.absolute();

    let mut buf = [0; 10];
    let buf = buf.as_mut_ptr();
    let mut buf_idx = 0;

    while buf_idx < 10 && idx < max && (!val.is_zero() || buf_idx < L) {
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
