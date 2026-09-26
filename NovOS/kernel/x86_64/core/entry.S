.section .text
.global kernel_entry
.extern kernel_main

kernel_entry:
    /* RDI contains BootInfo* (passed by bootloader) */
    /* Set up stack */
    lea kernel_stack_top(%rip), %rsp
    and $-16, %rsp
    xor %rbp, %rbp
    /* RDI already contains BootInfo*, call kernel_main(BootInfo*) */
    call kernel_main
.Lhang:
    hlt
    jmp .Lhang
