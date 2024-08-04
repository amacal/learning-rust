use adma_heap::Droplet;

use super::syscall::*;
use super::*;

impl IORing {
    fn shutdown_ref(&self) -> Result<(), IORingError> {
        let mut failed = false;

        failed = failed || 0 != sys_munmap(self.tx.sq_ptr, self.tx.sq_ptr_len);
        failed = failed || 0 != sys_munmap(self.tx.sq_sqes as *mut (), self.tx.sq_sqes_len);
        failed = failed || 0 != sys_munmap(self.rx.cq_ptr, self.rx.cq_ptr_len);
        failed = failed || 0 > sys_close(self.fd);

        if !failed {
            return Ok(());
        }

        Err(IORingError::ReleaseFailed)
    }

    pub fn shutdown(self) -> Result<(), IORingError> {
        self.shutdown_ref()
    }

    pub fn droplet(self) -> Droplet<IORing> {
        fn shutdown(ring: &mut IORing) {
            // tracing releases uring may help in any naive troubleshooting
            trace1(b"releasing uring droplet; fd=%d\n", ring.fd);

            if let Err(_) = ring.shutdown_ref() {
                // use tracing because we cannot propagate any error
                trace1(b"releasing uring droplet; failed, fd=%d\n", ring.fd);
            }
        }

        // tracing conversion ring to a droplet may help in any naive troubleshooting
        trace1(b"creating uring as droplet; fd=%d\n", self.fd);
        Droplet::from(self, shutdown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_ring() {
        let ring = match IORing::init(8) {
            Ok(ring) => ring.droplet(),
            _ => return assert!(false),
        };

        drop(ring);
    }
}
