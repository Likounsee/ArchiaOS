.text
.globl novos_x86_64_handoff
.type novos_x86_64_handoff,@function

novos_x86_64_handoff:
    /* UEFI/bootloader side uses Microsoft x64 ABI: RCX=entry, RDX=BootInfo. */
    /* NovOS kernel uses SysV AMD64 ABI: RDI=BootInfo. */
    movq %rcx, %rax
    movq %rdx, %rdi
    jmp *%rax
