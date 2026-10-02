.section .bss
.align 16
.global tss_stack
tss_stack:
    .zero 4096 * 256
.global tss_stack_top
tss_stack_top:

.align 16
.global tss_ist1_stack
tss_ist1_stack:
    .zero 4096 * 256
.global tss_ist1_stack_top
tss_ist1_stack_top:

.align 16
.global tss_ist2_stack
tss_ist2_stack:
    .zero 4096 * 256
.global tss_ist2_stack_top
tss_ist2_stack_top:

.align 16
.global tss_ist3_stack
tss_ist3_stack:
    .zero 4096 * 256
.global tss_ist3_stack_top
tss_ist3_stack_top:

.align 16
.global tss64
tss64:
    .zero 104 * 256

.section .text
.global tss_initialize
.global tss_initialize_cpu
.global tss_load
.global tss_load_cpu
.global tss_current_selector
.type tss_initialize, @function
tss_initialize:
    xor %edi, %edi
    call tss_initialize_cpu
    xor %edi, %edi
    jmp tss_load_cpu

.type tss_initialize_cpu, @function
tss_initialize_cpu:
    /* TSS and stack storage are laid out per CPU in 104/4096-byte slots. */
    mov %edi, %eax
    imul $104, %eax, %eax
    lea tss64(%rip), %rdx
    add %rax, %rdx

    mov %edi, %eax
    imul $4096, %eax, %eax
    lea tss_stack(%rip), %rcx
    add %rax, %rcx
    lea 4096(%rcx), %rax
    mov %rax, 4(%rdx)

    lea tss_ist1_stack(%rip), %rcx
    mov %edi, %eax
    imul $4096, %eax, %eax
    add %rax, %rcx
    lea 4096(%rcx), %rax
    mov %rax, 36(%rdx)

    lea tss_ist2_stack(%rip), %rcx
    mov %edi, %eax
    imul $4096, %eax, %eax
    add %rax, %rcx
    lea 4096(%rcx), %rax
    mov %rax, 44(%rdx)

    lea tss_ist3_stack(%rip), %rcx
    mov %edi, %eax
    imul $4096, %eax, %eax
    add %rax, %rcx
    lea 4096(%rcx), %rax
    mov %rax, 52(%rdx)

    /* I/O bitmap base immediately follows the 104-byte TSS. */
    movw $104, 102(%rdx)
    ret

.type tss_load, @function
tss_load:
    xor %edi, %edi
    jmp tss_load_cpu

.type tss_load_cpu, @function
tss_load_cpu:
    mov $0x28, %ax
    ltr %ax
    ret

.type tss_current_selector, @function
tss_current_selector:
    xor %eax, %eax
    str %ax
    ret
