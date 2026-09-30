.text
.globl archiaos_x86_64_handoff
.type archiaos_x86_64_handoff,@function

archiaos_x86_64_handoff:
    /*
     * UEFI/bootloader C++ side uses Microsoft x64 ABI:
     *   RCX = kernel entry
     *   RDX = BootInfo
     *
     * Kernel entry uses SysV AMD64:
     *   RDI = BootInfo
     */
    cli
    cld
    movq %rcx, %rax
    movq %rdx, %rdi
    jmp *%rax
