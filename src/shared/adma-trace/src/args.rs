use ::core::ptr;

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
