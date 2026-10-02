.section .text
.global ring3_enter
.type ring3_enter, @function
ring3_enter:
    cli
    mov %rsi, %rax
    andq $-16, %rax
    pushq $0x23
    pushq %rax
    pushfq
    orq $0x200, (%rsp)
    pushq $0x1B
    pushq %rdi
    iretq
.size ring3_enter, .-ring3_enter
