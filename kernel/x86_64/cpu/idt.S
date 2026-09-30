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

.macro ISR_NOERR n
.global isr\n
.type isr\n, @function
isr\n:
    pushq $0
    pushq $\n
    jmp isr_common
.endm

.macro ISR_ERR n
.global isr\n
.type isr\n, @function
isr\n:
    pushq $\n
    jmp isr_common
.endm

ISR_NOERR 0
ISR_NOERR 1
ISR_NOERR 2
ISR_NOERR 3
ISR_NOERR 4
ISR_NOERR 5
ISR_NOERR 6
ISR_NOERR 7
ISR_NOERR 8
ISR_NOERR 9
ISR_ERR   10
ISR_ERR   11
ISR_ERR   12
ISR_ERR   13
ISR_ERR   14
ISR_NOERR 15
ISR_NOERR 16
ISR_ERR   17
ISR_NOERR 18
ISR_NOERR 19
ISR_NOERR 20
ISR_ERR   21
ISR_NOERR 22
ISR_NOERR 23
ISR_NOERR 24
ISR_NOERR 25
ISR_NOERR 26
ISR_NOERR 27
ISR_NOERR 28
ISR_ERR   29
ISR_ERR   30
ISR_NOERR 31

ISR_NOERR 32
ISR_NOERR 33
ISR_NOERR 34
ISR_NOERR 35
ISR_NOERR 36
ISR_NOERR 37
ISR_NOERR 38
ISR_NOERR 39
ISR_NOERR 40
ISR_NOERR 41
ISR_NOERR 42
ISR_NOERR 43
ISR_NOERR 44
ISR_NOERR 45
ISR_NOERR 46
ISR_NOERR 47
ISR_NOERR 255

.global isr_common
.type isr_common, @function
isr_common:
    pushq %rax
    pushq %rbx
    pushq %rcx
    pushq %rdx
    pushq %rbp
    pushq %rdi
    pushq %rsi
    pushq %r8
    pushq %r9
    pushq %r10
    pushq %r11
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    mov %rsp, %rdi
    mov %rsp, %r11
    andq $-16, %rsp
    subq $8, %rsp
    call exception_dispatch
    mov %r11, %rsp

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %r11
    popq %r10
    popq %r9
    popq %r8
    popq %rsi
    popq %rdi
    popq %rbp
    popq %rdx
    popq %rcx
    popq %rbx
    popq %rax

    add $16, %rsp
    iretq

.global idt_initialize
.type idt_initialize, @function
idt_initialize:
    lea idt_table(%rip), %rdi

    lea isr0(%rip), %rax
    call idt_set_gate0
    lea isr1(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr2(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr3(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr4(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr5(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr6(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr7(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr8(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    movb $0x01, 4(%rdi)
    lea isr9(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr10(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr11(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr12(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr13(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr14(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr15(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr16(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr17(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr18(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr19(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr20(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr21(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr22(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr23(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr24(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr25(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr26(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr27(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr28(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr29(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr30(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr31(%rip), %rax
    add $16, %rdi
    call idt_set_gate0

    lea isr32(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr33(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr34(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr35(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr36(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr37(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr38(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr39(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr40(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr41(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr42(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr43(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr44(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr45(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr46(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea isr47(%rip), %rax
    add $16, %rdi
    call idt_set_gate0
    lea idt_table+4080(%rip), %rdi
    lea isr255(%rip), %rax
    call idt_set_gate0

    lea idt_pointer(%rip), %rax
    lidt (%rax)
    ret

.type idt_set_gate0, @function
idt_set_gate0:
    mov %rax, %rdx
    mov %dx, 0(%rdi)
    shr $16, %rdx
    mov %dx, 6(%rdi)
    shr $16, %rdx
    mov %edx, 8(%rdi)
    movw $0x0008, 2(%rdi)
    movb $0x00, 4(%rdi)
    movb $0x8E, 5(%rdi)
    ret

.global idt_test_invalid_opcode
.type idt_test_invalid_opcode, @function
idt_test_invalid_opcode:
    .byte 0x0F, 0x0B
    ret

.global idt_test_double_fault
.type idt_test_double_fault, @function
idt_test_double_fault:
    /* Deliberately use a non-canonical stack address. The following access
       raises #GP, and the CPU cannot push that exception frame, so #DF is
       delivered on IST1. */
    movabsq $0x0000800000000000, %rsp
    movq (%rsp), %rax
    ret
