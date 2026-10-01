ENTRY(kernel_entry)

PHDRS
{
    text PT_LOAD FLAGS(5);
    rodata PT_LOAD FLAGS(4);
    data PT_LOAD FLAGS(6);
}

SECTIONS
{
    /* Final kernel VMA; the UEFI loader may place the image anywhere physically. */
    . = 0xFFFFFFFF80100000;

    .text : ALIGN(4096)
    {
        __text_start = .;
        *(.text*)
        __text_end = .;
    } :text

    .rodata : ALIGN(4096)
    {
        __rodata_start = .;
        *(.rodata*)
        __rodata_end = .;
    } :rodata

    .data : ALIGN(4096)
    {
        __data_start = .;
        *(.data*)
        KEEP(*(.init_array*))
        __data_end = .;
    } :data

    .bss : ALIGN(4096)
    {
        __bss_start = .;
        *(COMMON)
        *(.bss*)
        __bss_end = .;
        . = ALIGN(4096);
        kernel_stack_guard = .;
        . += 0x1000;
        kernel_stack_bottom = .;
        . += 0x4000;
        kernel_stack_top = .;
    } :data

    /DISCARD/ :
    {
        *(.eh_frame*)
        *(.comment*)
        *(.note*)
    }
}
