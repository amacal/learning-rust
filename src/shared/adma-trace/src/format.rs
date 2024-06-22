#[macro_export]
macro_rules! format {
    ($name:ident, $(($idx:tt, $val:ident, $T:ident)),*) => {
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
