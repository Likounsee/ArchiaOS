.text
.globl x86_64_handoff

x86_64_handoff:
    /*
     * UEFI/bootloader C++ side uses Microsoft x64 ABI:
     *   RCX = kernel entry
     *   RDX = BootInfo
     *   R8  = temporary boot PML4 physical address
     *
     * Kernel entry uses SysV AMD64:
     *   RDI = BootInfo
     */
    cli
    cld
    movq %rcx, %rax
    movq %rdx, %rdi
    movq %r8, %cr3
    jmp *%rax
