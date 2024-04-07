pub fn itoa(buf: &mut [u8], value: i32, base: i32) -> Option<usize> {
    let mut value = value;
    let mut offset = 0;

    let negative = value < 0;
    let mut digits = [0u8; 11];

    if value == 0 {
        let value = match buf.get_mut(0) {
            None => return None,
            Some(value) => value,
        };

        *value = b'0';
        return Some(1);
    }

    while value != 0 {
        let digit = match digits.get_mut(offset) {
            None => return None,
            Some(value) => value,
        };

        *digit = match (value % base) as u8 {
            value if value < 10 => value + b'0',
            value => value - 10 + b'a',
        };

        offset += 1;
        value /= base;
    }

    if negative {
        let digit = match digits.get_mut(offset) {
            None => return None,
            Some(value) => value,
        };

        *digit = b'-';
        offset += 1;
    }

    for i in 0..offset {
        let value = match buf.get_mut(i) {
            None => return None,
            Some(value) => value,
        };

        *value = match digits.get(offset - i - 1) {
            None => return None,
            Some(value) => *value,
        };
    }

    Some(offset)
}

#[macro_export]
macro_rules! printf {
    ($fmt:expr, $($args:expr),*) => {{
        use crate::glibc::itoa;
        use crate::syscall::sys_write;

        let fmt = $fmt;
        let fmt_len = fmt.len();

        let mut buf = [0; 40];        ;
        let mut buf_idx = 0;

        let mut inside = false;
        let args = [$($args),*];
        let mut args_idx = 0;

        let base10 = 10;
        let base16 = 16;

        for i in 0..fmt_len {
            if !inside && fmt[i] == b'%' {
                inside = true;
            } else if inside && fmt[i] == b'd' {
                if let Some(value) = args.get(args_idx) {
                    let itoa = match &mut buf.get_mut(buf_idx..) {
                        Some(buf) => itoa(buf, *value, base10),
                        None => break,
                    };

                    match itoa {
                        Some(count) => buf_idx += count,
                        None => break,
                    }

                    args_idx += 1;
                }
                inside = false;
            } else if inside && fmt[i] == b'x' {
                if let Some(value) = args.get(args_idx) {
                    let itoa = match &mut buf.get_mut(buf_idx..) {
                        Some(buf) => itoa(buf, *value, base16),
                        None => break,
                    };

                    match itoa {
                        Some(count) => buf_idx += count,
                        None => break,
                    }

                    args_idx += 1;
                }
                inside = false;
            } else {
                match buf.get_mut(buf_idx) {
                    Some(value) => *value = fmt[i],
                    None => break,
                }
                buf_idx += 1;
            }
        }

        sys_write(2, buf.as_ptr(), buf_idx);
    }}
}
