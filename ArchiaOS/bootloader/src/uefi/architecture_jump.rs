#[cfg(target_arch = "x86_64")]
unsafe fn jump_to_kernel(entry: u64, info: *const BootInfo, page_table_root: u64) -> ! {
    // RDI carries BootInfo. High-half images receive fresh identity + kernel mappings;
    // identity test kernels keep firmware paging by passing root=0. Before using
    // the new NX-marked mappings, enable EFER.NXE; CR0.WP also makes supervisor
    // writes respect read-only kernel text/rodata pages.
    unsafe {
        core::arch::asm!(
            "cli",
            "cld",
            // Save ABI inputs before COM1 setup clobbers RAX/RDX.
            "mov r8, rax",
            "mov r9, rdx",
            // No UEFI calls after ExitBootServices. 'P' marks trampoline entry;
            // 'T' means execution continued after the optional CR3 switch.
            "mov dx, 0x3f9",
            "xor al, al",
            "out dx, al",
            "mov dx, 0x3fb",
            "mov al, 0x80",
            "out dx, al",
            "mov dx, 0x3f8",
            "mov al, 0x03",
            "out dx, al",
            "mov dx, 0x3f9",
            "xor al, al",
            "out dx, al",
            "mov dx, 0x3fb",
            "mov al, 0x03",
            "out dx, al",
            "mov dx, 0x3fa",
            "mov al, 0xc7",
            "out dx, al",
            "mov dx, 0x3fc",
            "mov al, 0x0b",
            "out dx, al",
            "mov dx, 0x3f8",
            "mov al, 0x50",
            "out dx, al",
            "mov al, 0x0d",
            "out dx, al",
            "mov al, 0x0a",
            "out dx, al",
            "test r9, r9",
            "jz 2f",
            // R8/R9 survive RDMSR, which overwrites EDX:EAX.
            "mov ecx, 0xC0000080",
            "rdmsr",
            "or eax, 0x800",
            "wrmsr",
            "mov rax, cr0",
            "or rax, 0x10000",
            "mov cr0, rax",
            "mov cr3, r9",
            "2:",
            "mov dx, 0x3f8",
            "mov al, 0x54",
            "out dx, al",
            "mov al, 0x0d",
            "out dx, al",
            "mov al, 0x0a",
            "out dx, al",
            "mov rax, r8",
            "jmp rax",
            in("rax") entry,
            in("rdi") info,
            in("rdx") page_table_root,
            // This assembly never returns to Rust. The register clobbers are
            // therefore immaterial to Rust's caller, while RAX/RDX inputs are
            // explicitly copied to R8/R9 before RDMSR overwrites them.
            options(noreturn)
        )
    }
}

#[cfg(target_arch = "aarch64")]
unsafe fn jump_to_kernel(entry: u64, info: *const BootInfo) -> ! {
    // AArch64 kernel entry receives its first argument in x0 (AAPCS64).
    unsafe {
        core::arch::asm!(
            "msr daifset, #0xf",
            "br x1",
            in("x0") info,
            in("x1") entry,
            options(noreturn)
        )
    }
}
