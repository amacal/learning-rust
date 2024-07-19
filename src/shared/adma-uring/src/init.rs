use ::core::ptr;

use super::kernel::*;
use super::syscall::*;
use super::*;

pub enum IORingInit {
    Succeeded(IORingSubmitter, IORingCompleter),
    InvalidDescriptor(isize),
    SetupFailed(isize),
    MappingFailed(&'static [u8], isize),
}

impl IORing {
    pub fn init(entries: u32) -> IORingInit {
        let mut params: io_uring_params = io_uring_params::default();
        let fd: u32 = match sys_io_uring_setup(entries, &mut params as *mut io_uring_params) {
            value if value < 0 => return IORingInit::SetupFailed(value),
            value => match value.try_into() {
                Err(_) => return IORingInit::InvalidDescriptor(value),
                Ok(value) => value,
            },
        };

        fn map<T>(fd: u32, array: u32, count: u32, offset: usize) -> (isize, usize) {
            let array = array as usize;
            let size = core::mem::size_of::<T>() as usize;

            let addr = ptr::null_mut();
            let length = array + size * count as usize;

            let prot = PROT_READ | PROT_WRITE;
            let flags = MAP_SHARED | MAP_POPULATE;

            (sys_mmap(addr, length, prot, flags, fd as usize, offset), length)
        }

        let sq_array = params.sq_off.array;
        let sq_entries = params.sq_entries;

        let offset = IORING_OFF_SQ_RING;
        let (sq_ptr, sq_ptr_len) = match map::<u32>(fd, sq_array, sq_entries, offset) {
            (res, _) if res <= 0 => return IORingInit::MappingFailed(b"SQ_RING", res),
            (res, len) => (res as *mut (), len),
        };

        let sq_tail = (sq_ptr as usize + params.sq_off.tail as usize) as *mut u32;
        let sq_array = (sq_ptr as usize + params.sq_off.array as usize) as *mut u32;
        let sq_ring_mask = (sq_ptr as usize + params.sq_off.ring_mask as usize) as *mut u32;
        let sq_ring_entries = (sq_ptr as usize + params.sq_off.ring_entries as usize) as *mut u32;

        trace2(b"ring ready; fd=%d, sq=%d\n", fd, unsafe { *sq_ring_entries });

        let offset = IORING_OFF_SQES;
        let (sq_sqes, sq_sqes_len) = match map::<io_uring_sqe>(fd, 0, sq_entries, offset) {
            (res, _) if res <= 0 => return IORingInit::MappingFailed(b"SQ_SQES", res),
            (res, len) => (res as *mut io_uring_sqe, len),
        };
        let cq_array = params.cq_off.cqes;
        let cq_entries = params.cq_entries;

        let offset = IORING_OFF_CQ_RING;
        let (cq_ptr, cq_ptr_len) = match map::<io_uring_cqe>(fd, cq_array, cq_entries, offset) {
            (res, _) if res <= 0 => return IORingInit::MappingFailed(b"CQ_RING", res),
            (res, len) => (res as *mut (), len),
        };

        let cq_head = (cq_ptr as usize + params.cq_off.head as usize) as *mut u32;
        let cq_tail = (cq_ptr as usize + params.cq_off.tail as usize) as *mut u32;
        let cq_cqes = (sq_ptr as usize + params.cq_off.cqes as usize) as *mut io_uring_cqe;
        let cq_ring_mask = (cq_ptr as usize + params.cq_off.ring_mask as usize) as *mut u32;
        let cq_ring_entries = (cq_ptr as usize + params.cq_off.ring_entries as usize) as *mut u32;

        trace2(b"ring ready; fd=%d, cq=%d\n", fd, unsafe { *cq_ring_entries });

        let submitter = IORingSubmitter {
            fd: fd,
            cnt_total: 0,
            cnt_queued: 0,
            sq_ptr: sq_ptr,
            sq_ptr_len: sq_ptr_len,
            sq_tail: sq_tail,
            sq_ring_mask: sq_ring_mask,
            sq_array: sq_array,
            sq_sqes: sq_sqes,
            sq_sqes_len: sq_sqes_len,
        };

        let completer = IORingCompleter {
            fd: fd,
            cq_ptr: cq_ptr,
            cq_ptr_len: cq_ptr_len,
            cq_head: cq_head,
            cq_tail: cq_tail,
            cq_ring_mask: cq_ring_mask,
            cq_cqes: cq_cqes,
        };

        IORingInit::Succeeded(submitter, completer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_new_ring_rx() {
        let (rx, tx) = match IORing::init(8) {
            IORingInit::Succeeded(tx, rx) => (rx, tx),
            _ => return assert!(false),
        };

        unsafe {
            assert_ne!(rx.fd, 0);
            assert_eq!(tx.fd, rx.fd);

            assert_ne!(rx.cq_ptr_len, 0);
            assert_ne!(rx.cq_ptr, ptr::null_mut());

            assert_ne!(rx.cq_ring_mask, ptr::null_mut());
            assert_ne!(*rx.cq_ring_mask, 0);

            assert_ne!(rx.cq_head, ptr::null_mut());
            assert_ne!(rx.cq_tail, ptr::null_mut());
            assert_ne!(rx.cq_cqes, ptr::null_mut());
        }
    }

    #[test]
    fn init_new_ring_tx() {
        let (rx, tx) = match IORing::init(8) {
            IORingInit::Succeeded(tx, rx) => (rx, tx),
            _ => return assert!(false),
        };

        unsafe {
            assert_ne!(rx.fd, 0);
            assert_eq!(tx.fd, rx.fd);

            assert_ne!(tx.sq_ptr_len, 0);
            assert_ne!(tx.sq_ptr, ptr::null_mut());

            assert_ne!(tx.sq_ring_mask, ptr::null_mut());
            assert_ne!(*tx.sq_ring_mask, 0);

            assert_ne!(tx.sq_array, ptr::null_mut());
            assert_ne!(tx.sq_tail, ptr::null_mut());

            assert_ne!(tx.sq_sqes, ptr::null_mut());
            assert_ne!(tx.sq_sqes_len, 0);
        }
    }
}
