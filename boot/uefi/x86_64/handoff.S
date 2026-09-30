.text
.globl archiaos_x86_64_handoff

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
    movb $72, %al
    outb %al, $0xE9
    movq %rcx, %rax
    movq %rdx, %rdi
    jmp *%rax
