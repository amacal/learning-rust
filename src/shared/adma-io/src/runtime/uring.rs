use crate::uring::*;

struct IORuntimeSubmitter<const T: usize> {
    uring: IORingSubmitter,
}
