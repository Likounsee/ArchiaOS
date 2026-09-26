.section .bss
.align 16
.global tss_stack
tss_stack:
    .zero 4096
.global tss_stack_top
tss_stack_top:

.section .data
.align 16
.global tss64
tss64:
    .zero 104

.section .text
.global tss_initialize
.type tss_initialize, @function
tss_initialize:
    lea tss_stack_top(%rip), %rax
    mov %rax, 4 + 4(%rip)
    shr $32, %rax
    mov %eax, 12 + 4(%rip)

    lea tss64(%rip), %rax
    mov %ax, 2(%rip)
    ret
