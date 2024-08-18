pub const P_ALL: i32 = 0;
pub const P_PID: i32 = 1;
pub const P_PIDFD: i32 = 3;

pub const F_GETFL: u32 = 3;
pub const F_SETFL: u32 = 4;

pub const O_NONBLOCK: u64 = 0x00000800;
pub const O_DIRECT: u64 = 0x00004000;

pub const EAGAIN: isize = -11;
pub const SIGPIPE: i32 = 13;

pub const SIG_DFL: usize = 0x00;
pub const SIG_IGN: usize = 0x01;

pub const PROT_READ: usize = 0x00000001;
pub const PROT_WRITE: usize = 0x00000002;
pub const MAP_PRIVATE: usize = 0x00000002;
pub const MAP_ANONYMOUS: usize = 0x00000020;

pub const CLONE_VM: u64 = 0x00000100;
pub const CLONE_FS: u64 = 0x00000200;
pub const CLONE_FILES: u64 = 0x00000400;
pub const CLONE_SIGHAND: u64 = 0x00000800;
pub const CLONE_PIDFD: u64 = 0x00001000;
pub const CLONE_THREAD: u64 = 0x00010000;

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

#[repr(C)]
#[allow(dead_code)]
#[allow(non_camel_case_types)]
pub struct sigaction {
    pub sa_sigaction: usize,
    pub sa_flags: i32,
    pub sa_restorer: usize,
    pub sa_mask: u64,
}
