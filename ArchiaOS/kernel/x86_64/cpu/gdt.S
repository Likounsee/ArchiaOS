.section .bss
.align 16
.global gdt_tables
gdt_tables:
    .zero 256 * 56

.align 16
.global gdt_pointers
gdt_pointers:
    .zero 256 * 10

.section .text
.global gdt_load_current
.global gdt_initialize
.global gdt_load_cpu
.global gdt_initialize_cpu
.type gdt_load_current, @function
gdt_load_current:
    xor %edi, %edi
    jmp gdt_load_cpu

.type gdt_load_cpu, @function
gdt_load_cpu:
    /* One GDT is 5 entries = 40 bytes; one pointer is 10 bytes. */
    mov %edi, %eax
    imul $56, %eax, %eax
    lea gdt_tables(%rip), %rdx
    add %rax, %rdx

    mov %edi, %eax
    imul $10, %eax, %eax
    lea gdt_pointers(%rip), %rcx
    add %rax, %rcx

    movw $55, (%rcx)
    mov %rdx, 2(%rcx)
    lgdt (%rcx)

    pushq $0x08
    leaq .Lreload_cs(%rip), %rax
    pushq %rax
    lretq
.Lreload_cs:
    mov $0x10, %ax
    mov %ax, %ds
    mov %ax, %es
    mov %ax, %ss
    xor %eax, %eax
    mov %ax, %fs
    mov %ax, %gs
    ret

.type gdt_initialize, @function
gdt_initialize:
    xor %edi, %edi
    call gdt_initialize_cpu
    xor %edi, %edi
    jmp gdt_load_cpu

.type gdt_initialize_cpu, @function
gdt_initialize_cpu:
    /*
     * Per-CPU GDT layout:
     *   0x00 null
     *   0x08 64-bit kernel code
     *   0x10 kernel data
     *   0x18 64-bit user code (DPL3)
     *   0x20 64-bit user data (DPL3)
     *   0x28 low 64-bit TSS descriptor
     *   0x30 high 64-bit TSS descriptor
     */
    mov %edi, %eax
    imul $40, %eax, %eax
    lea gdt_tables(%rip), %rcx
    add %rax, %rcx

    /* Clear the complete GDT before rebuilding its descriptors. */
    movq $0, 0(%rcx)
    movq $0, 8(%rcx)
    movq $0, 16(%rcx)
    movq $0, 24(%rcx)
    movq $0, 32(%rcx)
    movq $0, 40(%rcx)
    movq $0, 48(%rcx)
    movq $0, 48(%rcx)

    movabsq $0x00AF9A000000FFFF, %rax
    movq %rax, 8(%rcx)
    movabsq $0x00CF92000000FFFF, %rax
    movq %rax, 16(%rcx)
    movabsq $0x00AFFA000000FFFF, %rax
    movq %rax, 24(%rcx)
    movabsq $0x00CFF2000000FFFF, %rax
    movq %rax, 32(%rcx)

    /* The per-CPU TSS array is 104 bytes per CPU. */
    mov %edi, %eax
    imul $104, %eax, %eax
    lea tss64(%rip), %rdx
    add %rax, %rdx

    /* 64-bit TSS descriptor, limit = 103, type = available 64-bit TSS. */
    movw $103, 40(%rcx)
    mov %rdx, %rax
    mov %ax, 42(%rcx)
    shr $16, %rax
    mov %al, 44(%rcx)
    mov %rdx, %rax
    shr $24, %rax
    mov %al, 47(%rcx)
    movb $0x89, 45(%rcx)
    movb $0x00, 46(%rcx)
    mov %rdx, %rax
    shr $32, %rax
    mov %eax, 48(%rcx)
    ret

.size gdt_load_current, .-gdt_load_current
