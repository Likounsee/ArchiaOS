.section .multiboot,"a"
.align 4
.long 0x1BADB002
.long 0x00000003
.long -(0x1BADB002 + 0x00000003)

.section .text.boot,"ax"
.code32
.global _start
.type _start, @function
_start:
    cli
    cld
    mov esp, offset boot_stack_top

    /* Build 512 2 MiB identity mappings (first 1 GiB). */
    mov edi, offset page_directory
    xor ecx, ecx
1:
    mov eax, ecx
    shl eax, 21
    or eax, 0x83
    mov dword ptr [edi + ecx*8], eax
    mov dword ptr [edi + ecx*8 + 4], 0
    inc ecx
    cmp ecx, 512
    jne 1b

    mov eax, offset page_directory
    or eax, 0x03
    mov dword ptr [pdpt], eax
    mov dword ptr [pdpt + 4], 0
    mov eax, offset pdpt
    or eax, 0x03
    mov dword ptr [pml4], eax
    mov dword ptr [pml4 + 4], 0

    mov eax, offset pml4
    mov cr3, eax
    mov eax, cr4
    or eax, (1 << 5) | (1 << 9) | (1 << 10)
    mov cr4, eax

    mov ecx, 0xC0000080
    rdmsr
    or eax, (1 << 8)
    wrmsr
    mov eax, cr0
    and eax, 0xFFFFFFF3
    or eax, 0x80000002
    mov cr0, eax

    lgdt [gdt64_pointer]
    push 0x08
    push offset long_mode_start
    retf

.code64
long_mode_start:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax
    xor ax, ax
    mov fs, ax
    mov gs, ax
    mov rsp, offset boot_stack_top
    xor rbp, rbp
    call rust_kernel_main
2:
    cli
    hlt
    jmp 2b

.align 8
gdt64:
    .quad 0x0000000000000000
    .quad 0x00AF9A000000FFFF
    .quad 0x00CF92000000FFFF
gdt64_end:
gdt64_pointer:
    .word gdt64_end - gdt64 - 1
    .long gdt64

.section .bss,"aw",@nobits
.align 4096
pml4: .skip 4096
.align 4096
pdpt: .skip 4096
.align 4096
page_directory: .skip 4096
.align 16
boot_stack_bottom: .skip 16384
boot_stack_top:
