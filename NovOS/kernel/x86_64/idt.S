.section .bss
.align 16
idt_table:
    .zero 256 * 16

.section .rodata
.align 8
idt_pointer:
    .word (256 * 16) - 1
    .quad idt_table

.section .text
.global idt_initialize
.type idt_initialize, @function

idt_initialize:
    lea idt_pointer(%rip), %rax
    lidt (%rax)
    ret

.global idt_test_exception
.type idt_test_exception, @function

idt_test_exception:
    ud2
