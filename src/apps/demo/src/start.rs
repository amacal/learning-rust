use ::core::arch;

arch::global_asm!(
    "
    .global _start;

    _start:
        mov rdi, [rsp];
        lea rsi, [rsp + 8];
        push rsi;
        push rdi;
        mov rdi, rsp;
        call main;
"
);
