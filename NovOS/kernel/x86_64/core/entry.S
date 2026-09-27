.section .text
.global kernel_entry
.extern kernel_main

kernel_entry:
    /* RDI contains BootInfo* (passed by bootloader) */
    /* Set up stack with System V AMD64 ABI alignment:
       RSP must be 8 mod 16 immediately before CALL. */
    lea kernel_stack_top(%rip), %rsp
    and $-16, %rsp
    sub $8, %rsp
    xor %rbp, %rbp
    /* RDI still contains BootInfo* */
    call kernel_main
.Lhang:
    hlt
    jmp .Lhang
