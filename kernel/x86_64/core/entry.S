.text
.globl kernel_entry
.type kernel_entry,@function
.extern kernel_main

kernel_entry:
    movb $69, %al
    outb %al, $0xE9
    /* SysV AMD64 ABI: RDI already contains BootInfo*. */
    lea kernel_stack_top(%rip), %rsp
    andq $-16, %rsp
    xorq %rbp, %rbp
    call kernel_main

.Lhalt:
    cli
    hlt
    jmp .Lhalt
