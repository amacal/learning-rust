use ::core::ptr;

use super::kernel::*;
use super::syscall::*;
use super::trace::*;

use super::IORingSubmitEntry;
use super::IORingSubmitter;

pub enum IORingSubmit {
    Succeeded(usize),
    SubmissionFailed(isize),
    SubmissionMismatched(usize),
}

impl IORingSubmitter {
    pub fn submit<const C: usize>(&mut self, user_data: u64, entries: [IORingSubmitEntry; C]) -> IORingSubmit {
        let to_submit = entries.len();

        for entry in entries.into_iter() {
            let ring_mask = unsafe { ptr::read_volatile(self.sq_ring_mask) };
            let sq_tail = unsafe { ptr::read_volatile(self.sq_tail) & ring_mask };

            let (opcode, fd, ptr, len, offset) = match entry {
                IORingSubmitEntry::Noop() => {
                    /* fmt */
                    (IORING_OP_NOP, 0, ptr::null(), 0, 0)
                }
                IORingSubmitEntry::Timeout(data) => {
                    /* fmt */
                    (IORING_OP_TIMEOUT, 0, data.timespec as *const u8, 1, 0)
                }
                IORingSubmitEntry::OpenAt(data) => {
                    /* fmt */
                    (IORING_OP_OPENAT, data.fd, data.buf, 0, 0)
                }
                IORingSubmitEntry::Close(data) => {
                    /* fmt */
                    (IORING_OP_CLOSE, data.fd, ptr::null(), 0, 0)
                }
                IORingSubmitEntry::Read(data) => {
                    /* fmt */
                    (IORING_OP_READ, data.fd, data.buf, data.len, data.off)
                }
                IORingSubmitEntry::Write(data) => {
                    /* fmt */
                    (IORING_OP_WRITE, data.fd, data.buf, data.len, data.off)
                }
            };

            unsafe {
                self.cnt_total += 1;
                self.cnt_queued += 1;

                let sqe = self.sq_sqes.offset(sq_tail as isize);

                trace4(
                    b"submitting ring operation; op=%d, user=%d, total=%d, queued=%d\n",
                    opcode,
                    user_data,
                    self.cnt_total,
                    self.cnt_queued,
                );

                (*sqe).opcode = opcode;
                (*sqe).fd = fd as i32;
                (*sqe).addr = ptr as u64;
                (*sqe).len = len as u32;
                (*sqe).off = offset;
                (*sqe).user_data = user_data;

                ptr::write_volatile(self.sq_array.add(sq_tail as usize), sq_tail);
                ptr::write_volatile(self.sq_tail, (sq_tail + 1) & ring_mask);
            }
        }

        IORingSubmit::Succeeded(to_submit)
    }

    pub fn flush(&mut self) -> IORingSubmit {
        let min_complete = 0;
        let to_submit = self.cnt_queued as u32;

        trace2(b"flushing ring operation; total=%d, queued=%d\n", self.cnt_total, to_submit);

        if to_submit > 0 {
            let submitted = match sys_io_uring_enter(self.fd, to_submit, min_complete, 0, ptr::null(), 0) {
                value if value < 0 => return IORingSubmit::SubmissionFailed(value),
                value => value as usize,
            };

            self.cnt_queued = 0;

            if submitted != to_submit as usize {
                return IORingSubmit::SubmissionMismatched(submitted);
            }
        }

        IORingSubmit::Succeeded(to_submit as usize)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn submits_noop() {
        let mut ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            _ => return assert!(false),
        };

        match ring.tx.submit(13, [IORingSubmitEntry::noop()]) {
            IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
            IORingSubmit::SubmissionFailed(_) => assert!(false),
            IORingSubmit::SubmissionMismatched(_) => assert!(false),
        }

        match ring.tx.flush() {
            IORingSubmit::Succeeded(cnt) => assert_eq!(cnt, 1),
            IORingSubmit::SubmissionFailed(_) => assert!(false),
            IORingSubmit::SubmissionMismatched(_) => assert!(false),
        }
    }
}
