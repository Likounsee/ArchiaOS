.section .text.smp_trampoline,"ax"
.global smp_trampoline_start
.global smp_trampoline_end
.type smp_trampoline_start,@function

.set SMP_MAILBOX_CR3, 0x200
.set SMP_MAILBOX_ENTRY, 0x208
.set SMP_MAILBOX_STACK, 0x210
.set SMP_MAILBOX_INDEX, 0x218
.set SMP_MAILBOX_APIC_ID, 0x21C
.set SMP_MAILBOX_STARTED, 0x220

.code16
smp_trampoline_start:
    movb $0x41, %al
    movw $0xE9, %dx
    outb %al, %dx
    cli
    cld

    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    xorw %bx, %bx
    movw %cs, %bx
    shll $4, %ebx

    movl %ebx, %eax
    addl $(smp_trampoline_gdt - smp_trampoline_start), %eax

    movw $(smp_trampoline_gdtr - smp_trampoline_start), %di
    movl %eax, 2(%di)
    lgdt (%di)
    movb $0x45, %al
    movw $0xE9, %dx
    outb %al, %dx

    movl %cr0, %eax
    orl $1, %eax
    movl %eax, %cr0
    movb $0x46, %al
    movw $0xE9, %dx
    outb %al, %dx

    /*
     * The protected-mode entry is inside this 4 KiB trampoline page, so a
     * 16-bit far offset is sufficient and avoids relying on 32-bit far-jump
     * encoding while the CPU is still in real mode.
     */
    .byte 0xEA
    .word (smp_trampoline_pmode - smp_trampoline_start)
    .word 0x0008

.code32
smp_trampoline_pmode:
    movb $0x43, %al
    movw $0xE9, %dx
    outb %al, %dx

    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    movl %cr4, %eax
    orl $(1 << 5), %eax
    movl %eax, %cr4

    movl SMP_MAILBOX_CR3(%ebx), %eax
    movl %eax, %cr3

    movl $0xC0000080, %ecx
    rdmsr
    orl $(1 << 8), %eax
    wrmsr

    movl %cr0, %eax
    orl $(1 << 31), %eax
    movl %eax, %cr0

    movb $0x44, %al
    movw $0xE9, %dx
    outb %al, %dx

    .byte 0xEA
    .long (smp_trampoline_lmode - smp_trampoline_start)
    .word 0x0018

.code64
smp_trampoline_lmode:
    movb $0x42, %al
    movw $0xE9, %dx
    outb %al, %dx

    movw $0x20, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    movq SMP_MAILBOX_STACK(%rbx), %rsp
    andq $-16, %rsp

    movl $1, SMP_MAILBOX_STARTED(%rbx)

    movl SMP_MAILBOX_INDEX(%rbx), %edi
    movl SMP_MAILBOX_APIC_ID(%rbx), %esi
    movq SMP_MAILBOX_ENTRY(%rbx), %rax
    call *%rax

smp_trampoline_halt:
    cli
    hlt
    jmp smp_trampoline_halt

    .p2align 3
smp_trampoline_gdtr:
    .word 0x27
    .quad 0

smp_trampoline_gdt:
    .quad 0x0000000000000000
    .quad 0x00CF9A000000FFFF
    .quad 0x00CF92000000FFFF
    .quad 0x00AF9A000000FFFF
    .quad 0x00AF92000000FFFF

    .space 0x100

smp_trampoline_end:
.size smp_trampoline_start, smp_trampoline_end-smp_trampoline_start
