ENTRY(_start)

KERNEL_VMA = 0xffffffff80000000;
/* Avoid the first MiB above 1 MiB, which some UEFI firmwares do not make
 * available for AllocateAddress even when the remaining RAM is conventional.
 * Keep the image 2 MiB-aligned for predictable page-table mapping. */
KERNEL_LMA = 0x00200000;

PHDRS
{
    metadata PT_LOAD FLAGS(4);
    text     PT_LOAD FLAGS(5);
    rodata   PT_LOAD FLAGS(4);
    data     PT_LOAD FLAGS(6);
}

SECTIONS
{
    . = KERNEL_VMA + KERNEL_LMA;

    .archiaos_metadata ALIGN(8) : AT(ADDR(.archiaos_metadata) - KERNEL_VMA)
    {
        KEEP(*(.archiaos_image_marker))
    } :metadata

    .text ALIGN(4K) : AT(ADDR(.text) - KERNEL_VMA)
    {
        *(.text.boot)
        *(.text .text.*)
    } :text

    .rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_VMA)
    {
        *(.rodata .rodata.*)
    } :rodata

    .data ALIGN(4K) : AT(ADDR(.data) - KERNEL_VMA)
    {
        *(.data .data.*)
        /* Keep GOT entries used by optimized Rust code (e.g. memset). */
        *(.got .got.*)
    } :data

    .bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_VMA)
    {
        *(COMMON)
        *(.bss .bss.*)
    } :data

    /DISCARD/ :
    {
        *(.eh_frame .eh_frame.*)
        *(.comment)
    }

    ASSERT(LOADADDR(.text) >= KERNEL_LMA, "kernel load address is below the reserved base")
    ASSERT(LOADADDR(.bss) < 0x40000000, "kernel physical image must remain below 1 GiB")
}