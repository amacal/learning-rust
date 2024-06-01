pub const P_ALL: i32 = 0;
pub const P_PID: i32 = 1;
pub const P_PIDFD: i32 = 3;

pub const F_GETFL: u32 = 3;
pub const F_SETFL: u32 = 4;

pub const O_NONBLOCK: u32 = 0x0800;
pub const O_DIRECT: u32 = 0x4000;

pub const CLONE_VM: u64 = 0x00000100;
pub const CLONE_FS: u64 = 0x00000200;
pub const CLONE_FILES: u64 = 0x00000400;
pub const CLONE_SIGHAND: u64 = 0x00000800;
pub const CLONE_PIDFD: u64 = 0x00001000;
pub const CLONE_THREAD: u64 = 0x00010000;

#[repr(C)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
#[allow(non_camel_case_types)]
pub struct io_uring_cqe {
    pub user_data: u64,
    pub res: i32,
    pub flags: u32,
}

#[repr(C)]
#[allow(dead_code)]
#[allow(non_camel_case_types)]
pub struct timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

#[repr(C)]
#[allow(dead_code)]
#[allow(non_camel_case_types)]
pub struct clone_args {
    pub flags: u64,
    pub pidfd: u64,
    pub child_tid: u64,
    pub parent_tid: u64,
    pub exit_signal: u64,
    pub stack: u64,
    pub stack_size: u64,
    pub tls: u64,
    pub set_tid: u64,
    pub set_tid_size: u64,
    pub cgroup: u64,
}

#[repr(C)]
#[allow(dead_code)]
#[allow(non_camel_case_types)]
pub struct siginfo {
    pub si_signo: i32,
    pub si_errno: i32,
    pub si_code: i32,
    _padding: [u8; 116],
}

impl Default for siginfo {
    fn default() -> Self {
        Self {
            si_signo: 0,
            si_errno: 0,
            si_code: 0,
            _padding: [0; 116],
        }
    }
}
