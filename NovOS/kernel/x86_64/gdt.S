.section .rodata
.align 8
gdt_table:
    .quad 0x0000000000000000
    .quad 0x00AF9A000000FFFF
    .quad 0x00CF92000000FFFF

gdt_pointer:
    .word (3 * 8) - 1
    .quad gdt_table

.section .text
.global gdt_initialize
.type gdt_initialize, @function

gdt_initialize:
    lgdt gdt_pointer(%rip)

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
