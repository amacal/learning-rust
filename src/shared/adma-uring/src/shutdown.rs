use super::syscall::*;
use super::IORing;

pub enum IORingShutdown {
    Succeeded(),
    Failed(),
}

impl IORing {
    pub fn shutdown(self) -> IORingShutdown {
        let mut failed = false;

        failed = failed || 0 != sys_munmap(self.sq_ptr, self.sq_ptr_len);
        failed = failed || 0 != sys_munmap(self.sq_sqes as *mut (), self.sq_sqes_len);
        failed = failed || 0 != sys_munmap(self.cq_ptr, self.cq_ptr_len);
        failed = failed || 0 > sys_close(self.fd);

        if failed {
            IORingShutdown::Failed()
        } else {
            IORingShutdown::Succeeded()
        }
    }
}
