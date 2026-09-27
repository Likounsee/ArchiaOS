.text
.globl novos_x86_64_handoff

novos_x86_64_handoff:
    /* C++ caller uses Microsoft x64 ABI: RCX=kernelEntry, RDX=BootInfo. */
    /* NovOS kernel entry uses SysV AMD64 ABI: RDI=BootInfo. */
    cli
    cld
    movq %rcx, %rax
    movq %rdx, %rdi
    jmp *%rax
