.section .text.smp_trampoline,"ax"
.global smp_trampoline_start
.global smp_trampoline_end
.type smp_trampoline_start,@function

.code16
smp_trampoline_start:
    movb $0x41, %al
    movw $0xE9, %dx
    outb %al, %dx
    cli

smp_trampoline_halt:
    hlt
    jmp smp_trampoline_halt

smp_trampoline_end:
.size smp_trampoline_start, smp_trampoline_end-smp_trampoline_start
