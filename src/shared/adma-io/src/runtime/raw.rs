use ::core::ptr;
use ::core::task::*;

fn clone_fn(data: *const ()) -> RawWaker {
    RawWaker::new(data, IORING_WAKER_VTABLE)
}

pub fn make_waker() -> RawWaker {
    RawWaker::new(ptr::null(), IORING_WAKER_VTABLE)
}

fn wake_fn(_: *const ()) {}
fn wake_by_ref_fn(_: *const ()) {}
fn drop_fn(_: *const ()) {}

static IORING_WAKER_VTABLE: &RawWakerVTable = &RawWakerVTable::new(clone_fn, wake_fn, wake_by_ref_fn, drop_fn);
