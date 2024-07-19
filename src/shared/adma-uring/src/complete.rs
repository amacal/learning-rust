use ::core::ptr;

use super::kernel::*;
use super::syscall::*;

use super::IORingCompleter;

#[derive(Default, Clone, Copy)]
pub struct IORingCompleteEntry {
    pub res: i32,
    pub flags: u32,
    pub user_data: u64,
}

pub enum IORingComplete {
    Succeeded(usize),
    UnexpectedEmpty(usize),
    CompletionFailed(isize),
}

impl IORingCompleter {
    fn extract<const T: usize>(&self, entries: &mut [IORingCompleteEntry; T]) -> usize {
        let ring_mask = unsafe { ptr::read_volatile(self.cq_ring_mask) };
        let mut cnt = 0;

        while cnt < T {
            let cq_head = unsafe { ptr::read_volatile(self.cq_head) };
            let cq_tail = unsafe { ptr::read_volatile(self.cq_tail) };

            if cq_head == cq_tail {
                return cnt;
            }

            let index = cq_head & ring_mask;
            let entry = unsafe { self.cq_cqes.offset(index as isize) };

            entries[cnt] = IORingCompleteEntry {
                res: unsafe { (*entry).res },
                flags: unsafe { (*entry).flags },
                user_data: unsafe { (*entry).user_data },
            };

            cnt += 1;
            unsafe { ptr::write_volatile(self.cq_head, cq_head + 1) };
        }

        cnt
    }

    pub fn complete<const T: usize>(&self, entries: &mut [IORingCompleteEntry; T]) -> IORingComplete {
        let cnt = self.extract(entries);
        if cnt > 0 {
            return IORingComplete::Succeeded(cnt);
        }

        let to_submit = 0;
        let min_complete = 1;
        let flags = IORING_ENTER_GETEVENTS;

        let count = match sys_io_uring_enter(self.fd, to_submit, min_complete, flags, ptr::null(), 0) {
            value if value < 0 => return IORingComplete::CompletionFailed(value),
            value => value as usize,
        };

        let cnt = self.extract(entries);

        if cnt > 0 {
            IORingComplete::Succeeded(cnt)
        } else {
            IORingComplete::UnexpectedEmpty(count)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn receives_completion() {
        let (rx, mut tx) = match IORing::init(8) {
            IORingInit::Succeeded(tx, rx) => (rx, tx),
            _ => return assert!(false),
        };

        let null = b"/dev/null\0";
        let entry = IORingSubmitEntry::open_at(null.as_ptr());

        match tx.submit(13, [entry]) {
            IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
            IORingSubmit::SubmissionFailed(_) => assert!(false),
            IORingSubmit::SubmissionMismatched(_) => assert!(false),
        }

        match tx.flush() {
            IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
            IORingSubmit::SubmissionFailed(_) => assert!(false),
            IORingSubmit::SubmissionMismatched(_) => assert!(false),
        }

        let mut entries = [IORingCompleteEntry::default(); 1];
        match rx.complete(&mut entries) {
            IORingComplete::Succeeded(cnt) => assert_eq!(cnt, 1),
            IORingComplete::UnexpectedEmpty(_) => assert!(false),
            IORingComplete::CompletionFailed(_) => assert!(false),
        }
    }
}
