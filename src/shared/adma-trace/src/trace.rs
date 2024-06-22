#[macro_export]
macro_rules! trace {
    ($name:ident, $format:ident, $feature:literal, $(($idx:tt, $val:ident, $T:ident)),+) => {
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

            $format(&mut msg, fmt, $($val),*);
            sys_write(2, msg.as_ptr() as *const (), msg.len());
        }
    };
}
