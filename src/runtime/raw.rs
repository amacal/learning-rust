use core::task::{RawWaker, RawWakerVTable};

fn clone_fn(data: *const ()) -> RawWaker {
    RawWaker::new(data, IORING_WAKER_VTABLE)
}

pub fn make_waker(data: *const ()) -> RawWaker {
    RawWaker::new(data, IORING_WAKER_VTABLE)
}

fn wake_fn(_: *const ()) {}
fn wake_by_ref_fn(_: *const ()) {}
fn drop_fn(_: *const ()) {}

static IORING_WAKER_VTABLE: &RawWakerVTable = &RawWakerVTable::new(clone_fn, wake_fn, wake_by_ref_fn, drop_fn);
