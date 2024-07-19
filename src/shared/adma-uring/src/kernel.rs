pub const IORING_OFF_SQ_RING: usize = 0x00000000;
pub const IORING_OFF_CQ_RING: usize = 0x08000000;
pub const IORING_OFF_SQES: usize = 0x010000000;
pub const IORING_ENTER_GETEVENTS: u32 = 0x00000001;

pub const IORING_OP_NOP: u8 = 0;
pub const IORING_OP_TIMEOUT: u8 = 11;
pub const IORING_OP_OPENAT: u8 = 18;
pub const IORING_OP_CLOSE: u8 = 19;
pub const IORING_OP_READ: u8 = 22;
pub const IORING_OP_WRITE: u8 = 23;

pub const AT_FDCWD: i32 = -100;

pub const PROT_READ: usize = 0x00000001;
pub const PROT_WRITE: usize = 0x00000002;

pub const MAP_SHARED: usize = 0x00000001;
pub const MAP_POPULATE: usize = 0x00008000;

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Default)]
pub struct io_sqring_offsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub flags: u32,
    pub dropped: u32,
    pub array: u32,
    pub resv1: u32,
    pub user_addr: u64,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Default)]
pub struct io_cqring_offsets {
    pub head: u32,
    pub tail: u32,
    pub ring_mask: u32,
    pub ring_entries: u32,
    pub overflow: u32,
    pub cqes: u32,
    pub flags: u32,
    pub resv1: u32,
    pub user_addr: u64,
}

#[repr(C)]
#[allow(non_camel_case_types)]
#[derive(Default)]
pub struct io_uring_params {
    pub sq_entries: u32,
    pub cq_entries: u32,
    pub flags: u32,
    pub sq_thread_cpu: u32,
    pub sq_thread_idle: u32,
    pub features: u32,
    pub wq_fd: u32,
    pub resv: [u32; 3],
    pub sq_off: io_sqring_offsets,
    pub cq_off: io_cqring_offsets,
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct io_uring_sqe {
    pub opcode: u8,
    pub flags: u8,
    pub ioprio: u16,
    pub fd: i32,
    pub off: u64,
    pub addr: u64,
    pub len: u32,
    pub rw_flags: u32,
    pub user_data: u64,
    pub buf_index: u16,
    pub personality: u16,
    pub splice_fd_in: i32,
    pub __pad2: [u64; 2],
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct io_uring_cqe {
    pub user_data: u64,
    pub res: i32,
    pub flags: u32,
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}
