.section .rodata
.align 8
gdt_table:
    .quad 0x0000000000000000
    .quad 0x00AF9A000000FFFF
    .quad 0x00CF92000000FFFF
    .quad 0x0000000000000000
    .quad 0x0000000000000000

gdt_pointer:
    .word (5 * 8) - 1
    .quad gdt_table

.section .text
.global gdt_initialize
.type gdt_initialize, @function

gdt_initialize:
    lea tss64(%rip), %rax

    mov %rax, %rdx
    mov %dx, gdt_table+24(%rip)

    shr $16, %rdx
    mov %dx, gdt_table+26(%rip)

    shr $16, %rdx
    mov %dl, gdt_table+28(%rip)

    shr $8, %rdx
    mov %dl, gdt_table+29(%rip)

    movb $0x89, gdt_table+29(%rip)
    movb $0x00, gdt_table+30(%rip)

    mov %rax, %rdx
    shr $32, %rdx
    mov %dl, gdt_table+31(%rip)

    lea gdt_pointer(%rip), %rax
    lgdt (%rax)

    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %ss
    xor %eax, %eax
    mov %ax, %fs
    mov %ax, %gs

    pushq $0x08
    leaq .Lreload_cs(%rip), %rax
    pushq %rax
    lretq

.Lreload_cs:
    ret
