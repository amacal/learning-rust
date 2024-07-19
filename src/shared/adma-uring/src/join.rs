use super::{IORing, IORingCompleter, IORingSubmitter};

pub enum IORingJoin {
    Succeeded(IORing),
    MismatchedDescriptor(IORingSubmitter, IORingCompleter),
}

impl IORing {
    pub fn join(submitter: IORingSubmitter, completer: IORingCompleter) -> IORingJoin {
        if submitter.fd != completer.fd {
            return IORingJoin::MismatchedDescriptor(submitter, completer);
        }

        let ring = IORing {
            fd: submitter.fd,
            sq_ptr: submitter.sq_ptr,
            sq_ptr_len: submitter.sq_ptr_len,
            sq_sqes: submitter.sq_sqes,
            sq_sqes_len: submitter.sq_sqes_len,
            cq_ptr: completer.cq_ptr,
            cq_ptr_len: completer.cq_ptr_len,
        };

        IORingJoin::Succeeded(ring)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn join_successfully() {
        let (rx, tx) = match IORing::init(8) {
            IORingInit::Succeeded(tx, rx) => (rx, tx),
            _ => return assert!(false, "Cannot create I/O Ring."),
        };

        match IORing::join(tx, rx) {
            IORingJoin::Succeeded(_) => assert!(true),
            IORingJoin::MismatchedDescriptor(_, _) => assert!(false),
        }
    }

    #[test]
    fn join_failure() {
        let (rx1, tx1) = match IORing::init(8) {
            IORingInit::Succeeded(tx, rx) => (rx, tx),
            _ => return assert!(false, "Cannot create I/O Ring."),
        };

        let (rx2, tx2) = match IORing::init(8) {
            IORingInit::Succeeded(tx, rx) => (rx, tx),
            _ => return assert!(false, "Cannot create I/O Ring."),
        };

        match IORing::join(tx1, rx2) {
            IORingJoin::Succeeded(_) => return assert!(false),
            IORingJoin::MismatchedDescriptor(_, _) => assert!(true),
        }

        match IORing::join(tx2, rx1) {
            IORingJoin::Succeeded(_) => return assert!(false),
            IORingJoin::MismatchedDescriptor(_, _) => assert!(true),
        }
    }
}
