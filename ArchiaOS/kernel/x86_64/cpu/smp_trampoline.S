.section .text.smp_trampoline,"ax"
.global smp_trampoline_start
.global smp_trampoline_end
.type smp_trampoline_start,@function

.set SMP_MAILBOX_CR3,      0x400
.set SMP_MAILBOX_ENTRY,    0x408
.set SMP_MAILBOX_STACK,    0x410
.set SMP_MAILBOX_CR0,      0x418
.set SMP_MAILBOX_CR4,      0x420
.set SMP_MAILBOX_EFER,     0x428
.set SMP_MAILBOX_INDEX,    0x430
.set SMP_MAILBOX_APIC_ID,  0x434

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

    xorl %ebx, %ebx
    movw %cs, %bx
    shll $4, %ebx

    /* Build a real-mode 16:32 far pointer to protected-mode code. */
    movw $(smp_trampoline_pmode_ptr - smp_trampoline_start), %di
    movl %ebx, %eax
    addl $(smp_trampoline_pmode - smp_trampoline_start), %eax
    movl %eax, 0(%di)
    movw $0x0008, 4(%di)

    movl %ebx, %eax
    addl $(smp_trampoline_gdt - smp_trampoline_start), %eax
    movw $(smp_trampoline_gdtr - smp_trampoline_start), %di
    movl %eax, 2(%di)
    lgdt (%di)

    movl %cr0, %eax
    orl $1, %eax
    movl %eax, %cr0

    /* Operand-size override selects a 32-bit offset for this far jump. */
    .byte 0x66
    .byte 0xFF
    .byte 0x2E
    .word (smp_trampoline_pmode_ptr - smp_trampoline_start)

.code32
smp_trampoline_pmode:
    movb $0x43, %al
    movw $0xE9, %dx
    outb %al, %dx

    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss

    movb $0x45, %al
    movw $0xE9, %dx
    outb %al, %dx

    movl SMP_MAILBOX_CR4(%ebx), %eax
    orl $(1 << 5), %eax
    andl $~(1 << 17), %eax
    movl %eax, %cr4

    movb $0x46, %al
    movw $0xE9, %dx
    outb %al, %dx

    movl SMP_MAILBOX_CR3(%ebx), %eax
    movl %eax, %cr3

    movb $0x47, %al
    movw $0xE9, %dx
    outb %al, %dx

    movl SMP_MAILBOX_EFER(%ebx), %ecx
    movl %ecx, %eax
    xorl %edx, %edx
    movl $0xC0000080, %ecx
    wrmsr

    movb $0x48, %al
    movw $0xE9, %dx
    outb %al, %dx

    movl SMP_MAILBOX_CR0(%ebx), %eax
    orl $((1 << 0) | (1 << 31)), %eax
    andl $~((1 << 29) | (1 << 30)), %eax
    movl %eax, %cr0

    movb $0x49, %al
    movw $0xE9, %dx
    outb %al, %dx

    /* Build a protected-mode 16:32 far pointer to long-mode code. */
    movw $(smp_trampoline_lmode_ptr - smp_trampoline_start), %di
    movl %ebx, %eax
    addl $(smp_trampoline_lmode - smp_trampoline_start), %eax
    movl %eax, 0(%di)
    movw $0x0018, 4(%di)

    movb $0x44, %al
    movw $0xE9, %dx
    outb %al, %dx

    /* FF /5 with a 32-bit displacement: far jump through m16:32
       at trampoline_base + smp_trampoline_lmode_ptr. */
    .byte 0xFF
    .byte 0xAB
    .long (smp_trampoline_lmode_ptr - smp_trampoline_start)

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

    movl SMP_MAILBOX_INDEX(%rbx), %edi
    movl SMP_MAILBOX_APIC_ID(%rbx), %esi
    movq SMP_MAILBOX_ENTRY(%rbx), %rax
    testq %rax, %rax
    jz smp_trampoline_halt

    call *%rax

smp_trampoline_halt:
    cli
    hlt
    jmp smp_trampoline_halt

    .p2align 3
smp_trampoline_pmode_ptr:
    .long 0
    .word 0

smp_trampoline_lmode_ptr:
    .long 0
    .word 0

smp_trampoline_gdtr:
    .word 0x27
    .long 0

smp_trampoline_gdt:
    .quad 0x0000000000000000
    .quad 0x00CF9A000000FFFF
    .quad 0x00CF92000000FFFF
    .quad 0x00AF9A000000FFFF
    .quad 0x00AF92000000FFFF

    .space 0x100

smp_trampoline_end:
.size smp_trampoline_start, smp_trampoline_end-smp_trampoline_start
