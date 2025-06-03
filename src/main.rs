use std::arch::global_asm;
use std::{mem, ptr};

#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Uring {
    fd: u32, // 0

    tx_tail: usize, // 8
    tx_mask: usize, // 16
    tx_sqes: usize, // 24
    tx_indx: usize, // 32
    rx_head: usize, // 40
    rx_tail: usize, // 48
    rx_mask: usize, // 56
    rx_cqes: usize, // 64

    tx_ptr: usize,   // 72
    tx_len: usize,   // 80
    rx_ptr: usize,   // 88
    rx_len: usize,   // 96
    sqes_ptr: usize, // 104
    sqes_len: usize, // 112

    user_data: usize,     // 120
    user_data_ptr: usize, // 128
}

#[repr(C)]
pub struct Closure {
    ptr: extern "C" fn(*mut u8) -> u32,
    drop: extern "C" fn(*mut u8),
    data: *mut u8,
}

#[repr(align(64))]
pub struct Number {
    val: u32,
}

impl Drop for Number {
    fn drop(&mut self) {
        println!("Dropping...");
    }
}

impl Number {
    #[inline(never)]
    pub fn value(self) -> u32 {
        return self.val;
    }
}

fn create_closure<'a, TCallback>(callback: TCallback, dst: *mut u8) -> *mut Closure
where
    TCallback: FnOnce() -> u32 + Send + 'a,
{
    fn round_up(offset: usize, alignment: usize) -> usize {
        (offset + alignment - 1) & !(alignment - 1)
    }

    let offset = mem::size_of::<Closure>();
    let alignment = mem::align_of::<TCallback>();

    let size = round_up(offset, alignment);
    let target = unsafe { dst.add(size) };

    let closure: *mut TCallback = target as *mut TCallback;
    unsafe { ptr::write(closure, callback) };

    extern "C" fn invoke<TCallback>(closure: *mut u8) -> u32
    where
        TCallback: FnOnce() -> u32,
    {
        let closure: *const TCallback = closure as *const TCallback;
        let callback: TCallback = unsafe { ptr::read(closure) };

        return callback();
    }

    extern "C" fn release<TCallback>(closure: *mut u8)
    where
        TCallback: FnOnce() -> u32,
    {
        let closure: *const TCallback = closure as *const TCallback;
        let callback: TCallback = unsafe { ptr::read(closure) };

        drop(callback);
    }

    let ptr: *mut Closure = dst as *mut Closure;
    let instance = Closure { data: target, ptr: invoke::<TCallback>, drop: release::<TCallback> };

    unsafe { ptr::write(ptr, instance) };
    return ptr;
}

#[no_mangle]
pub extern "C" fn call_closure(closure: *mut Closure) -> u32 {
    unsafe {
        let closure: Closure = ptr::read(closure);
        let result: u32 = (closure.ptr)(closure.data);

        return result;
    }
}

#[no_mangle]
pub extern "C" fn drop_closure(closure: *mut Closure) {
    unsafe {
        let closure: Closure = ptr::read(closure);
        let result: () = (closure.drop)(closure.data);

        return result;
    }
}

extern "C" {
    fn alloc_stack() -> *mut u8;
    fn run_closure(uring: *const Uring, closure: *mut Closure, stack: *mut u8) -> u64;
    fn uring_noop(uring: *const Uring) -> isize;
    fn uring_init(uring: *mut Uring) -> isize;
    fn uring_loop(uring: *const Uring, iteration: u64) -> isize;
}

global_asm!(
    r#"
        alloc_stack:
            mov rdi, 0                  # addr = NULL
            mov rsi, 4096               # length = 4KB
            mov rdx, 0x03               # PROT_READ | PROT_WRITE
            mov r10, 0x22               # MAP_PRIVATE | MAP_ANONYMOUS
            mov r8, -1                  # fd = -1
            mov r9, 0                   # offset = 0
            mov rax, 9                  # mmap syscall
            syscall
            ret

        dealloc_stack:
            mov rsi, 4096               # length = 4KB
            mov rax, 11                 # unmmap syscall
            syscall
            ret

        run_closure:
            push rdi                    # remember ptr to uring struct
            push rsi                    # remember ptr to closure
            push rdx                    # remember ptr to stack

            mov r10d, [rdi]             # load uring fd as u32
            mov rdx, [rdi+8]            # load tx_tail ptr
            mov rcx, [rdi+16]           # load tx_mask ptr
            mov rsi, [rdi+32]           # load tx_indx ptr
            mov rdi, [rdi+24]           # load tx_sqes ptr

            mov ecx, [rcx]              # read mask value
            mov eax, [rdx]              # read tail index

            and rax, rcx                # mask for wrapping
            mov r11, rax                # remember current slot
            shl rax, 6                  # SQE size is 64 bytes
            add rdi, rax                # SQE item address

            xor rax, rax                # source value is 0
            mov rcx, 8                  # 8 iterations, each 8 bytes
            rep stosq                   # fill 64 bytes with 0
            sub rdi, 64                 # rewind to SQE item address

            mov rax, [rsp]              # load closure's stack, passed in RDX
            sub rax, 64                 # make space for saved registers + closure

            mov [rdi+32], rax           # remember RSP in user-data
            mov [rsi+r11*4], r11d       # store SQE slot in the ring
            inc dword ptr [rdx]         # increment tail indexs

            mov rdi, r10                # copy uring fd
            mov rsi, 1                  # 1 SQE
            xor rdx, rdx                # 0 CQE
            xor r10, r10                # no flags
            xor r8, r8                  # no sigset
            xor r9, r9                  # no sigset
            mov rax, 426                # io_uring_enter syscall
            syscall

        # execute continue polling

            pop rdx                     # restore and remove stack
            pop rsi                     # restore and remove closure
            pop rdi                     # restore and remove uring struct

            mov rax, [rip + run_closure_done@GOTPCREL]    # store jump address
            mov [rdx-8], rsi
            mov [rdx-16], rax
            mov [rdx-24], rbx
            mov [rdx-32], rdx
            mov [rdx-40], r12
            mov [rdx-48], r13
            mov [rdx-56], r14
            mov [rdx-64], r15

            ret

        .global run_closure_done;
        run_closure_done:
            mov rsi, rdi                # copy uring struct ptr
            pop rdi                     # restore ptr to closure

            call call_closure
            ud2

            mov rax, [rsi+128]          # load user-data ptr
            mov rsp, [rax]              # restore main RSP
            pop rbp                     # restore main RBP

            mov qword ptr [rax], 0      # reset main RSP
            mov rax, 13                 # return 13

            ret

        .global uring_init
        uring_init:
            push rdi                    # remember ptr to uring struct

            sub rsp, 136                # reserve 136 bytes on stack
            mov rcx, 17                 # 17 iterations, each 8 bytes
            xor rax, rax                # source value is 0
            mov rdi, rsp                # destination pointer
            rep stosq                   # fill 136 bytes with 0
            mov dword ptr [rsp], 32     # sq_entries set to 32

            mov rdi, 32                 # submission queue size
            mov rsi, rsp                # ptr to io_uring_params
            mov rax, 425                # io_uring_setup syscall
            syscall

            mov rdx, [rsp+136]          # copy from stack ptr to uring
            mov [rdx], eax              # store uring fd

            # map IORING_OFF_SQ_RING

            mov esi, [rsp]              # sq_entries
            shl esi, 6                  # * size of slot (4 bytes)
            add esi, [rsp+64]           # + sq_off.array

            xor rdi, rdi                # addr = NULL
            mov rdx, 0x0003             # PROT_READ | PROT_WRITE
            mov r10, 0x8001             # MAP_SHARED | MAP_POPULATE
            mov r8, rax                 # uring file descriptor
            xor r9, r9                  # IORING_OFF_SQ_RING
            mov rax, 9                  # mmap syscall
            syscall

            # copy IORING_OFF_SQ_RING pointers

            mov rdx, [rsp+136]          # copy from stack ptr to uring
            mov [rdx+72], rax           # tx_ptr
            mov [rdx+80], rsi           # tx_len

            mov rdi, rax                # base mapped address
            mov esi, [rsp+44]           # sq_off.tail
            add rdi, rsi                # sq_off.tail ptr
            mov [rdx+8], rdi            # store in tx_tail

            mov rdi, rax                # base mapped address
            mov esi, [rsp+48]           # sq_off.mask
            add rdi, rsi                # sq_off.mask ptr
            mov [rdx+16], rdi           # store in tx_mask

            mov rdi, rax                # base mapped address
            mov esi, [rsp+64]           # sq_off.array
            add rdi, rsi                # sq_off.array ptr
            mov [rdx+32], rdi           # store in tx_indx

            # map IORING_OFF_CQ_RING

            mov rdx, [rsp+136]          # copy from stack ptr to uring
            mov eax, [rdx]              # read uring fd

            mov esi, [rsp+4]            # cq_entries
            shl esi, 4                  # * size of CQE (16 bytes)
            add esi, [rsp+92]           # + cq_off.cqes

            xor rdi, rdi                # addr = NULL
            mov rdx, 0x0003             # PROT_READ | PROT_WRITE
            mov r10, 0x8001             # MAP_SHARED | MAP_POPULATE
            mov r8, rax                 # uring file descriptor
            mov r9, 0x08000000          # IORING_OFF_CQ_RING
            mov rax, 9                  # mmap syscall
            syscall

            # copy IORING_OFF_CQ_RING pointers

            mov rdx, [rsp+136]          # copy from stack ptr to uring
            mov [rdx+88], rax           # rx_ptr
            mov [rdx+96], rsi           # rx_len

            mov rdi, rax                # base mapped address
            mov esi, [rsp+80]           # cq_off.head
            add rdi, rsi                # cq_off.head ptr
            mov [rdx+40], rdi           # store in rx_head

            mov rdi, rax                # base mapped address
            mov esi, [rsp+84]           # cq_off.tail
            add rdi, rsi                # cq_off.tail ptr
            mov [rdx+48], rdi           # store in rx_tail

            mov rdi, rax                # base mapped address
            mov esi, [rsp+88]           # cq_off.mask
            add rdi, rsi                # cq_off.mask ptr
            mov [rdx+56], rdi           # store in rx_mask

            mov rdi, rax                # base mapped address
            mov esi, [rsp+100]          # cq_off.cqes
            add rdi, rsi                # cq_off.cqes ptr
            mov [rdx+64], rdi           # store in rx_cqes

            # map IORING_OFF_SQES

            mov rdx, [rsp+136]          # copy from stack ptr to uring
            mov eax, [rdx]              # read uring fd

            mov esi, [rsp]              # sq_entries
            shl esi, 6                  # * size of SQE (64 bytes)
            add esi, [rsp+92]           # + sq_off.sqes

            xor rdi, rdi                # addr = NULL
            mov rdx, 0x0003             # PROT_READ | PROT_WRITE
            mov r10, 0x8001             # MAP_SHARED | MAP_POPULATE
            mov r8, rax                 # uring file descriptor
            mov r9, 0x10000000          # IORING_OFF_SQES
            mov rax, 9                  # mmap syscall
            syscall

            # copy IORING_OFF_SQES pointers

            mov rdx, [rsp+136]          # copy from stack ptr to uring
            mov [rdx+104], rax          # sqes_ptr
            mov [rdx+112], rsi          # sqes_len

            mov rdi, rax                # base mapped address
            mov [rdx+24], rdi           # store in tx_sqes

            # user_data

            lea rax, [rdx+120]          # find ptr to user-data
            mov qword ptr[rdx+120], 0   # zero user-data content
            mov [rdx+128], rax          # store ptr to user-data

        .clean_up:
            mov rax, [rdx+56]
            mov eax, [rax]
            add rsp, 144                # revert local stack changes
            ret

        .global uring_loop;
        uring_loop:
            push rsi
            push rdi                    # remember ptr to uring struct

        .loop:
            mov rdi, [rsp]              # restore ptr to uring struct
            mov edi, [rdi]              # copy uring fd
            xor rsi, rsi                # 0 SQE
            mov rdx, 0x01               # 1 CQE
            mov r10, 0x01               # IORING_ENTER_GETEVENTS
            xor r8, r8                  # no sigset
            xor r9, r9                  # no sigset
            mov rax, 426                # io_uring_enter syscall
            syscall

            mov rdi, [rsp]              # restore ptr to uring struct
            mov rdx, [rdi+40]           # load rx_head
            mov r11, [rdi+48]           # load rx_tail
            mov rsi, [rdi+64]           # load rx_cqes
            mov rcx, [rdi+56]           # load rx_mask

            mov edx, [rdx]              # dereference head
            mov r11d, [r11]             # dereference tail
            mov ecx, [rcx]              # dereference mask
            cmp rdx, r11                # compare them
            je .loop                    # if equal then wait again

            and rdx, rcx                # mask head
            shl rdx, 4                  # CQE size is 16 bytes
            add rsi, rdx                # CQE address

            mov rdx, [rdi+40]           # load rx_head
            inc dword ptr [rdx]         # increment head index

            # here we continue execution behind user-data

            pop rdi                     # restore ptr to uring struct
            pop rdx

            push rbp                    # remember current RBP on stack
            mov rax, [rdi+128]          # load user-data ptr
            mov [rax], rsp              # store current RSP in uring struct
            mov rsp, [rsi+0]            # restore task's RSP from user-data

            pop r15
            pop r14
            pop r13
            pop r12
            pop rbp
            pop rbx

            pop rax                     # restore task continuation
            jmp rax                     # continue execution with RDI

        .global uring_noop;
        uring_noop:
            push rdi                    # remember ptr to uring struct

            mov r10d, [rdi]             # load uring fd as u32
            mov rdx, [rdi+8]            # load tx_tail ptr
            mov rcx, [rdi+16]           # load tx_mask ptr
            mov rsi, [rdi+32]           # load tx_indx ptr
            mov rdi, [rdi+24]           # load tx_sqes ptr

            mov ecx, [rcx]              # read mask value
            mov eax, [rdx]              # read tail index

            and rax, rcx                # mask for wrapping
            mov r11, rax                # remember current slot
            shl rax, 6                  # SQE size is 64 bytes
            add rdi, rax                # SQE item address

            xor rax, rax                # source value is 0
            mov rcx, 8                  # 8 iterations, each 8 bytes
            rep stosq                   # fill 64 bytes with 0
            sub rdi, 64                 # rewind to SQE item address

            mov rax, rsp                # load closure's stack
            sub rax, 48                 # make space for saved registers + closure
            mov [rdi+32], rax           # remember RSP in user-data
            mov [rsi+r11*4], r11d       # store SQE slot in the ring
            inc dword ptr [rdx]         # increment tail indexs

            mov rdi, r10                # copy uring fd
            mov rsi, 1                  # 1 SQE
            xor rdx, rdx                # 0 CQE
            xor r10, r10                # no flags
            xor r8, r8                  # no sigset
            xor r9, r9                  # no sigset
            mov rax, 426                # io_uring_enter syscall
            syscall

        # execute continue polling

            pop rdi                     # restore and remove uring struct
            mov rax, [rip + uring_noop_done@GOTPCREL]    # where to jump on done
            push rax                    # store jump address

            push rbx
            push rbp
            push r12
            push r13
            push r14
            push r15

            mov rax, [rdi+128]          # load user-data ptr
            mov rsp, [rax]              # restore main RSP
            pop rbp                     # restore main RBP

            mov rsi, 1
            jmp uring_loop              # jump to polling with uring struct in RDI, iteration in RSI

        .global uring_noop_done;
        uring_noop_done:
            mov rax, [rdi+128]          # load user-data ptr
            mov qword ptr [rax], 0      # reset main RSP
            mov rax, 0xff               # return 13

            ret
    "#
);

pub fn dump(header: &str, ptr: usize, len: usize) {
    print!("{}", header);

    for idx in 0..len {
        if idx % 32 == 0 {
            println!();
            print!("  ");
        }

        print!("{:02x} ", unsafe { *((ptr as *const u8).add(idx)) });
    }

    println!();
    println!();
}

pub fn main() {
    let mut uring = Uring::default();
    let val = unsafe { uring_init(&mut uring) };

    println!("U: {} {:?}", val, uring);

    let number = Number { val: 7 };
    let callback = move || {
        println!("before-noop");
        let val = unsafe { uring_noop(&uring) };
        println!("after-noop: {0:08x}", val);

        println!("before-noop");
        let val = unsafe { uring_noop(&uring) };
        println!("after-noop: {0:08x}", val);

        return number.value() + 13;
    };

    let data = unsafe { alloc_stack() };
    let ptr = create_closure(callback, data);

    println!("Pointer : {0:08x}", ptr as usize);
    println!("Data    : {0:08x}", unsafe { (*ptr).data as usize });

    let stack = unsafe { data.add(4096) };
    let val = unsafe { run_closure(&uring, ptr, stack) };

    println!("Value   : {0:08x}", val);

    let val = unsafe { uring_loop(&uring, 0) };
    println!("Loop    : {0:08x}", val);
}
