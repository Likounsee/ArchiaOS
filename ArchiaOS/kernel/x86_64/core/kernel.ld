ENTRY(kernel_entry)

SECTIONS
{
    . = 0x100000;

    .text : ALIGN(4096)
    {
        __text_start = .;
        *(.text*)
        __text_end = .;
    }

    .rodata : ALIGN(4096)
    {
        __rodata_start = .;
        *(.rodata*)
        __rodata_end = .;
    }

    .data : ALIGN(4096)
    {
        __data_start = .;
        *(.data*)
        __data_end = .;
    }

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
    }
}
