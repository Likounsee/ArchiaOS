ENTRY(kernel_entry)

SECTIONS
{
    . = 0x100000;

    .text : ALIGN(4096)
    {
        *(.text*)
    }

    .rodata : ALIGN(4096)
    {
        *(.rodata*)
    }

    .data : ALIGN(4096)
    {
        *(.data*)
    }

    .bss : ALIGN(4096)
    {
        *(COMMON)
        *(.bss*)
        . = ALIGN(16);
        kernel_stack_bottom = .;
        . += 0x4000;
        kernel_stack_top = .;
    }
}
