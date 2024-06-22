#![cfg_attr(not(feature = "std"), no_std)]

mod args;
mod format;
mod syscall;
mod trace;

pub use args::*;

#[allow(dead_code)]
#[cfg(not(feature = "tracing"))]
pub fn trace0(_fmt: &'static [u8]) {}

#[allow(dead_code)]
#[inline(never)]
#[cfg(feature = "tracing")]
pub fn trace0(fmt: &'static [u8]) {
    use crate::syscall::*;
    sys_write(2, fmt.as_ptr() as *const (), fmt.len());
}

format!(format1, (0, val0, T0));
format!(format2, (0, val0, T0), (1, val1, T1));
format!(format3, (0, val0, T0), (1, val1, T1), (2, val2, T2));
format!(format4, (0, val0, T0), (1, val1, T1), (2, val2, T2), (3, val3, T3));
format!(format5, (0, val0, T0), (1, val1, T1), (2, val2, T2), (3, val3, T3), (4, val4, T4));
format!(format6, (0, val0, T0), (1, val1, T1), (2, val2, T2), (3, val3, T3), (4, val4, T4), (5, val5, T5));

trace!(trace1, format1, "tracing", (0, val0, T0));
trace!(trace2, format2, "tracing", (0, val0, T0), (1, val1, T1));
trace!(trace3, format3, "tracing", (0, val0, T0), (1, val1, T1), (2, val2, T2));
trace!(trace4, format4, "tracing", (0, val0, T0), (1, val1, T1), (2, val2, T2), (3, val3, T3));
trace!(trace5, format5, "tracing", (0, val0, T0), (1, val1, T1), (2, val2, T2), (3, val3, T3), (4, val4, T4));
trace!(trace6, format6, "tracing", (0, val0, T0), (1, val1, T1), (2, val2, T2), (3, val3, T3), (4, val4, T4), (5, val5, T5));
