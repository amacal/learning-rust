use super::*;
use crate::runtime::file::*;

impl IORuntimeOps {
    pub fn stdout(&self) -> StdOutDescriptor {
        StdOutDescriptor::new()
    }
}
