.section .bss
.align 16
.global tss_stack
tss_stack:
    .zero 4096
.global tss_stack_top
tss_stack_top:

.align 16
.global tss_ist1_stack
tss_ist1_stack:
    .zero 4096
.global tss_ist1_stack_top
tss_ist1_stack_top:

.align 16
.global tss_ist2_stack
tss_ist2_stack:
    .zero 4096
.global tss_ist2_stack_top
tss_ist2_stack_top:

.align 16
.global tss_ist3_stack
tss_ist3_stack:
    .zero 4096
.global tss_ist3_stack_top
tss_ist3_stack_top:

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
    mov %rax, tss64+4(%rip)

    lea tss_ist1_stack_top(%rip), %rax
    mov %rax, tss64+36(%rip)

    lea tss_ist2_stack_top(%rip), %rax
    mov %rax, tss64+44(%rip)

    lea tss_ist3_stack_top(%rip), %rax
    mov %rax, tss64+52(%rip)

    /* 104-byte TSS: I/O bitmap starts immediately after the TSS,
       so no I/O-permission bitmap is present. */
    movw $104, tss64+102(%rip)

    mov $0x18, %ax
    ltr %ax
    ret
