use crate::sys_write;

pub struct SyncCommand {
    pub msg: &'static [u8],
}

impl SyncCommand {
    pub async fn execute(self) -> Option<&'static [u8]> {
        sys_write(1, self.msg.as_ptr() as *const (), self.msg.len());
        None
    }
}
