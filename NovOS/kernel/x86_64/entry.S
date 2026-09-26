.section .text
.global kernel_entry
.extern kernel_main

kernel_entry:
    lea kernel_stack_top(%rip), %rsp
    and $-16, %rsp
    xor %rbp, %rbp
    call kernel_main
.Lhang:
    hlt
    jmp .Lhang
