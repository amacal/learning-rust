use super::args::*;

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
            if neg && idx < max {
                *msg.add(idx) = b'-';
                idx += 1;
            }

            if zero && idx < max {
                *msg.add(idx) = b'0';
                idx += 1;
            }

            for i in (0..buf_idx).rev() {
                if idx < max {
                    *msg.add(idx) = *buf.add(i);
                    idx += 1;
                }
            }
        } else {
            if idx < max {
                *msg.add(idx) = b'?';
                idx += 1;
            }
        }
    }

    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_number_base10() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<i32, 10, 0>(ptr, 0, 8, 12345);

        assert_eq!(idx, 5);
        assert_eq!(msg, *b"12345\0\0\0");
    }

    #[test]
    fn format_number_base16() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<i32, 16, 0>(ptr, 0, 8, 12345);

        assert_eq!(idx, 4);
        assert_eq!(msg, *b"3039\0\0\0\0");
    }

    #[test]
    fn format_number_base10_zero() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<i32, 10, 0>(ptr, 0, 8, 0);

        assert_eq!(idx, 1);
        assert_eq!(msg, *b"0\0\0\0\0\0\0\0");
    }

    #[test]
    fn format_number_base16_zero() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<i32, 16, 0>(ptr, 0, 8, 0);

        assert_eq!(idx, 1);
        assert_eq!(msg, *b"0\0\0\0\0\0\0\0");
    }

    #[test]
    fn format_number_base10_padding() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<u32, 10, 7>(ptr, 0, 8, 12345);

        assert_eq!(idx, 7);
        assert_eq!(msg, *b"0012345\0");
    }

    #[test]
    fn format_number_base16_padding() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<u32, 16, 6>(ptr, 0, 8, 12345);

        assert_eq!(idx, 6);
        assert_eq!(msg, *b"003039\0\0");
    }

    #[test]
    fn format_number_base10_truncated_because_of_sign() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<i32, 10, 0>(ptr, 0, 8, -12345678);

        assert_eq!(idx, 8);
        assert_eq!(msg, *b"-1234567");
    }

    #[test]
    fn format_number_base10_truncated_because_of_value() {
        let mut msg = [b'\0'; 8];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<u32, 10, 0>(ptr, 0, 8, 123456780);

        assert_eq!(idx, 8);
        assert_eq!(msg, *b"12345678");
    }

    #[test]
    fn format_number_base16_truncated_because_of_value() {
        let mut msg = [b'\0'; 6];
        let ptr = msg.as_mut_ptr();
        let idx = format_number::<u32, 16, 0>(ptr, 0, 6, 123456780);

        assert_eq!(idx, 6);
        assert_eq!(msg, *b"75bcd0");
    }
}
