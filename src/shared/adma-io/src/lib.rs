#![cfg_attr(not(feature = "std"), no_std)]

#![feature(fn_traits)]
#![feature(waker_getters)]

pub mod core;
pub mod heap;
pub mod kernel;
pub mod pipe;
pub mod proc;
pub mod runtime;
pub mod sha1;
pub mod syscall;
pub mod trace;
pub mod uring;
