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
    lea idt_table(%rip), %rdi
    add $96, %rdi
    lea idt_exception6(%rip), %rax

    mov %rax, %rdx
    mov %dx, 0(%rdi)

    mov %rax, %rdx
    shr $16, %rdx
    mov %dx, 6(%rdi)

    mov %rax, %rdx
    shr $32, %rdx
    mov %edx, 8(%rdi)

    movw $0x0008, 2(%rdi)
    movb $0x00, 4(%rdi)
    movb $0x8E, 5(%rdi)

    lea idt_pointer(%rip), %rax
    lidt (%rax)
    ret

.global idt_test_exception
.type idt_test_exception, @function

idt_test_exception:
    ud2

.global idt_exception6
.type idt_exception6, @function

idt_exception6:
    cli
    mov $'I', %al
    out %al, $0xE9
    mov $'D', %al
    out %al, $0xE9
    mov $'T', %al
    out %al, $0xE9
    mov $' ', %al
    out %al, $0xE9
    mov $'#', %al
    out %al, $0xE9
    mov $'U', %al
    out %al, $0xE9
    mov $'D', %al
    out %al, $0xE9
    mov $'\n', %al
    out %al, $0xE9
.Lexception6_halt:
    hlt
    jmp .Lexception6_halt
