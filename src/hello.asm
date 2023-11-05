section .text
global sum_u8

sum_u8:
    movzx   eax, dil
    add     al, sil
    ret
