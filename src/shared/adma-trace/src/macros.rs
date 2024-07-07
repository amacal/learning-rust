#[macro_export]
macro_rules! format {
    ($name:ident $(,($idx:tt, $val:ident, $T:ident))*) => {
        #[allow(dead_code)]
        #[inline(never)]
        pub fn $name<const T: usize, $($T),*>(
            msg: &mut [u8; T],
            fmt: &'static [u8],
            $($val: $T),*
        ) -> usize  where $($T: FormatArg + Copy),* {
            let len = fmt.len();
            let fmt = fmt.as_ptr();

            let mut spec = false;
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
                            $(
                                ($idx, b'd') => match $val.to_number() {
                                    None => (val_idx + 1, msg_idx),
                                    Some(val) => (val_idx + 1, format_number::<_, 10, 0>(msg, msg_idx, T, val)),
                                },
                                ($idx, b'x') => match $val.to_number() {
                                    None => (val_idx + 1, msg_idx),
                                    Some(val) => (val_idx + 1, format_number::<_, 16, 8>(msg, msg_idx, T, val)),
                                },
                                ($idx, b's') => match $val.to_string() {
                                    None => (val_idx + 1, msg_idx),
                                    Some(val) => (val_idx + 1, format_string(msg, msg_idx, T, val)),
                                },
                            )*
                            _ => (val_idx + 1, msg_idx),
                        }
                    } else {
                        *msg.add(msg_idx) = *fmt.add(i);
                        msg_idx += 1;
                    }

                    if msg_idx == T {
                        break;
                    }
                }

                msg_idx
            }
        }
    };
}

#[macro_export]
macro_rules! trace {
    ($name:ident, $format:ident, $feature:literal $(,($idx:tt, $val:ident, $T:ident))*) => {
        #[allow(dead_code)]
        #[allow(unused_variables)]
        #[inline(never)]
        #[cfg(not(feature = $feature))]
        pub fn $name<$($T),*>(fmt: &'static [u8], $($val: $T),*)
        where $($T: FormatArg + Copy),* {
        }

        #[allow(dead_code)]
        #[inline(never)]
        #[cfg(feature = $feature)]
        pub fn $name<$($T),*>(fmt: &'static [u8], $($val: $T),*)
        where $($T: FormatArg + Copy),* {
            use crate::syscall::*;
            let mut msg: [u8; 80] = [0; 80];

            let len = $format(&mut msg, fmt, $($val),*);
            sys_write(2, msg.as_ptr() as *const (), len);
        }
    };
}

#[macro_export]
macro_rules! tracing {
    ($feature:literal) => {
        self::format!(format0);
        self::format!(format1, (0, v0, T0));
        self::format!(format2, (0, v0, T0), (1, v1, T1));
        self::format!(format3, (0, v0, T0), (1, v1, T1), (2, v2, T2));
        self::format!(format4, (0, v0, T0), (1, v1, T1), (2, v2, T2), (3, v3, T3));
        self::format!(format5, (0, v0, T0), (1, v1, T1), (2, v2, T2), (3, v3, T3), (4, v4, T4));
        self::format!(format6, (0, v0, T0), (1, v1, T1), (2, v2, T2), (3, v3, T3), (4, v4, T4), (5, v5, T5));

        self::trace!(trace0, format0, $feature);
        self::trace!(trace1, format1, $feature, (0, v0, T0));
        self::trace!(trace2, format2, $feature, (0, v0, T0), (1, v1, T1));
        self::trace!(trace3, format3, $feature, (0, v0, T0), (1, v1, T1), (2, v2, T2));
        self::trace!(trace4, format4, $feature, (0, v0, T0), (1, v1, T1), (2, v2, T2), (3, v3, T3));
        self::trace!(trace5, format5, $feature, (0, v0, T0), (1, v1, T1), (2, v2, T2), (3, v3, T3), (4, v4, T4));
        self::trace!(trace6, format6, $feature, (0, v0, T0), (1, v1, T1), (2, v2, T2), (3, v3, T3), (4, v4, T4), (5, v5, T5));
    };
}
