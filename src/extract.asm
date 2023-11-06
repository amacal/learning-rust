section .rodata
    align 32
    and_256 dd 0b0000_0001_0000_0001_0000_0001_0000_0001
            dd 0b0000_0010_0000_0010_0000_0010_0000_0010
            dd 0b0000_0100_0000_0100_0000_0100_0000_0100
            dd 0b0000_1000_0000_1000_0000_1000_0000_1000
            dd 0b0001_0000_0001_0000_0001_0000_0001_0000
            dd 0b0010_0000_0010_0000_0010_0000_0010_0000
            dd 0b0100_0000_0100_0000_0100_0000_0100_0000
            dd 0b1000_0000_1000_0000_1000_0000_1000_0000

    align 32
    shuffle_256 db 0, 4, 8, 12, 1, 5, 9, 13
                db 2, 6, 10, 14, 3, 7, 11, 15
                db 0, 4, 8, 12, 1, 5, 9, 13
                db 2, 6, 10, 14, 3, 7, 11, 15

    align 32
    permute_256 dd 0, 4, 1, 5, 2, 6, 3, 7

    align 32
    one_256 db 0x01

section .text
    global extract_bits

    ; Function to extract bits from bytes in 'src' to 'dst'
    ; Arguments:
    ; - rsi: pointer to destination u8 array (dst)
    ; - rsi: pointer to source u8 array (src)
    ; - rdx: number of bytes to extract

extract_bits:
    vmovdqa         ymm1, [rel and_256]
    vmovdqa         ymm2, [rel shuffle_256]
    vmovdqa         ymm3, [rel permute_256]
    vpxor           ymm4, ymm4, ymm4
    vpbroadcastb    ymm5, [rel one_256]

.scalar_check:
    test            rdx, rdx
    jz              .done

    test            rsi, 31
    jz              .simd_check

.scalar_head:
    movzx           eax, byte [rsi]

    test            al, 1
    setnz           [rdi]
    test            al, 2
    setnz           [rdi + 1]
    test            al, 4
    setnz           [rdi + 2]
    test            al, 8
    setnz           [rdi + 3]
    test            al, 16
    setnz           [rdi + 4]
    test            al, 32
    setnz           [rdi + 5]
    test            al, 64
    setnz           [rdi + 6]
    test            al, 128
    setnz           [rdi + 7]

    add             rsi, 1
    add             rdi, 8
    sub             rdx, 1
    jmp             .scalar_check

.simd_check:
    cmp             rdx, 32
    jl              .scalar_tail

.simd_loop:
    vpbroadcastd    ymm0, dword [rsi]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi], ymm0

    vpbroadcastd    ymm0, dword [rsi + 4]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi + 32], ymm0

    vpbroadcastd    ymm0, dword [rsi + 8]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi + 64], ymm0

    vpbroadcastd    ymm0, dword [rsi + 12]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi + 96], ymm0

    vpbroadcastd    ymm0, dword [rsi + 16]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi + 128], ymm0

    vpbroadcastd    ymm0, dword [rsi + 20]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi + 160], ymm0

    vpbroadcastd    ymm0, dword [rsi + 24]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi + 192], ymm0

    vpbroadcastd    ymm0, dword [rsi + 28]
    vpand           ymm0, ymm0, ymm1
    vpshufb         ymm0, ymm0, ymm2
    vpermd          ymm0, ymm3, ymm0
    vpcmpeqb        ymm0, ymm0, ymm4
    vpaddb          ymm0, ymm0, ymm5
    vmovdqa         [rdi + 224], ymm0

    add             rsi, 32
    add             rdi, 256
    sub             rdx, 32
    jmp             .simd_check

.done:
    vzeroupper
    ret

.scalar_tail:
    test            rdx, rdx
    jz              .done

    movzx           eax, byte [rsi]

    test            al, 1
    setnz           [rdi]
    test            al, 2
    setnz           [rdi + 1]
    test            al, 4
    setnz           [rdi + 2]
    test            al, 8
    setnz           [rdi + 3]
    test            al, 16
    setnz           [rdi + 4]
    test            al, 32
    setnz           [rdi + 5]
    test            al, 64
    setnz           [rdi + 6]
    test            al, 128
    setnz           [rdi + 7]

    add             rsi, 1
    add             rdi, 8
    sub             rdx, 1

    jmp .scalar_tail