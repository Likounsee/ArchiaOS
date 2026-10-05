cmake_minimum_required(VERSION 3.20)
project(ArchiaOS NONE)

set(ARCHIAOS_ROOT ${CMAKE_CURRENT_SOURCE_DIR})
set(ARCHIAOS_BUILD ${CMAKE_BINARY_DIR})

set(ARCHIAOS_CLANG "" CACHE FILEPATH "Clang++ compiler")
set(ARCHIAOS_LLD "" CACHE FILEPATH "LLD ELF linker")
set(ARCHIAOS_LLD_LINK "" CACHE FILEPATH "LLD PE/COFF linker")
option(ARCHIAOS_BOOT_DEBUG "Enable UEFI debug-port output" ON)

if(NOT ARCHIAOS_CLANG)
    find_program(ARCHIAOS_CLANG
        NAMES clang++.exe clang++ clang++-18 clang++-19 clang++-17
        PATHS /usr/bin /usr/local/bin)
endif()
if(NOT ARCHIAOS_LLD)
    find_program(ARCHIAOS_LLD
        NAMES ld.lld.exe ld.lld ld.lld-18 ld.lld-19 ld.lld-17
        PATHS /usr/bin /usr/local/bin)
endif()
if(NOT ARCHIAOS_LLD_LINK)
    find_program(ARCHIAOS_LLD_LINK
        NAMES lld-link.exe lld-link lld-link-18 lld-link-19 lld-link-17
        PATHS /usr/bin /usr/local/bin)
endif()

if(NOT ARCHIAOS_CLANG AND EXISTS "C:/Program Files/LLVM/bin/clang++.exe")
    set(ARCHIAOS_CLANG "C:/Program Files/LLVM/bin/clang++.exe")
endif()
if(NOT ARCHIAOS_LLD AND EXISTS "C:/Program Files/LLVM/bin/ld.lld.exe")
    set(ARCHIAOS_LLD "C:/Program Files/LLVM/bin/ld.lld.exe")
endif()
if(NOT ARCHIAOS_LLD_LINK AND EXISTS "C:/Program Files/LLVM/bin/lld-link.exe")
    set(ARCHIAOS_LLD_LINK "C:/Program Files/LLVM/bin/lld-link.exe")
endif()

foreach(tool IN ITEMS ARCHIAOS_CLANG ARCHIAOS_LLD ARCHIAOS_LLD_LINK)
    if(NOT ${tool})
        message(FATAL_ERROR "${tool} not found. Install LLVM or pass -D${tool}=...")
    endif()
endforeach()

set(ARCHIAOS_BOOT_DIR ${ARCHIAOS_ROOT}/boot/uefi/x86_64)
set(ARCHIAOS_BOOT_INC ${ARCHIAOS_ROOT}/boot/uefi/include)
set(ARCHIAOS_COMMON ${ARCHIAOS_ROOT}/common)
set(ARCHIAOS_KERNEL_DIR ${ARCHIAOS_ROOT}/kernel/x86_64/core)
set(ARCHIAOS_CPU_DIR ${ARCHIAOS_ROOT}/kernel/x86_64/cpu)
set(ARCHIAOS_MEMORY_DIR ${ARCHIAOS_ROOT}/kernel/x86_64/memory)
set(ARCHIAOS_PROCESS_DIR ${ARCHIAOS_ROOT}/kernel/x86_64/process)
set(ARCHIAOS_FS_DIR ${ARCHIAOS_ROOT}/kernel/x86_64/fs)
set(ARCHIAOS_DRIVERS_DIR ${ARCHIAOS_ROOT}/kernel/x86_64/drivers)
set(ARCHIAOS_OPENFS_ROOT ${ARCHIAOS_ROOT}/../third_party/OpenFS/OpenFS)
set(ARCHIAOS_OPENFS_INC ${ARCHIAOS_OPENFS_ROOT}/include)
set(ARCHIAOS_OPENFS_COMPAT ${ARCHIAOS_FS_DIR}/openfs_compat)

set(ARCHIAOS_BOOT_HEADERS
    ${ARCHIAOS_BOOT_INC}/uefi.h
    ${ARCHIAOS_BOOT_INC}/boot_debug.h
    ${ARCHIAOS_BOOT_INC}/filesystem.h
    ${ARCHIAOS_BOOT_INC}/elf_loader.h
    ${ARCHIAOS_BOOT_INC}/framebuffer.h
    ${ARCHIAOS_BOOT_INC}/memory_map.h
    ${ARCHIAOS_BOOT_INC}/handoff.h
    ${ARCHIAOS_BOOT_INC}/boot_paging.h
    ${ARCHIAOS_COMMON}/boot_info.h
)

set(ARCHIAOS_KERNEL_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel.obj)
set(ARCHIAOS_KERNEL_ENTRY_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_entry.obj)
set(ARCHIAOS_KERNEL_MEMORY_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_memory.obj)
set(ARCHIAOS_KERNEL_ACPI_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_acpi.obj)
set(ARCHIAOS_KERNEL_EXCEPTION_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_exception.obj)
set(ARCHIAOS_KERNEL_GDT_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_gdt.obj)
set(ARCHIAOS_KERNEL_IDT_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_idt.obj)
set(ARCHIAOS_KERNEL_IRQ_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_irq.obj)
set(ARCHIAOS_KERNEL_LAPIC_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_lapic.obj)
set(ARCHIAOS_KERNEL_IOAPIC_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_ioapic.obj)
set(ARCHIAOS_KERNEL_TSS_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_tss.obj)
set(ARCHIAOS_KERNEL_PAGING_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_paging.obj)
set(ARCHIAOS_KERNEL_PAGING_TEST_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_paging_test.obj)
set(ARCHIAOS_KERNEL_PMM_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_pmm.obj)
set(ARCHIAOS_KERNEL_PMM_TEST_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_pmm_test.obj)
set(ARCHIAOS_KERNEL_CPUID_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_cpuid.obj)
set(ARCHIAOS_KERNEL_CPU_REPORT_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_cpu_report.obj)
set(ARCHIAOS_KERNEL_SECURITY_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_security.obj)
set(ARCHIAOS_KERNEL_SMP_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_smp.obj)
set(ARCHIAOS_KERNEL_SCHEDULER_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_scheduler.obj)
set(ARCHIAOS_KERNEL_SMP_TRAMPOLINE_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_smp_trampoline.obj)
set(ARCHIAOS_KERNEL_VMM_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_vmm.obj)
set(ARCHIAOS_KERNEL_HEAP_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_heap.obj)
set(ARCHIAOS_KERNEL_ADDRESS_SPACE_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_address_space.obj)
set(ARCHIAOS_KERNEL_PROCESS_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_process.obj)
set(ARCHIAOS_KERNEL_RING3_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_ring3.obj)
set(ARCHIAOS_KERNEL_IPC_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_ipc.obj)
set(ARCHIAOS_KERNEL_VFS_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_vfs.obj)
set(ARCHIAOS_KERNEL_PCI_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_pci.obj)
set(ARCHIAOS_KERNEL_INPUT_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_input.obj)
set(ARCHIAOS_KERNEL_GRAPHICS_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_graphics.obj)
set(ARCHIAOS_KERNEL_GUI_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_gui.obj)
set(ARCHIAOS_KERNEL_DEVICE_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_device.obj)
set(ARCHIAOS_KERNEL_NET_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_net.obj)
set(ARCHIAOS_KERNEL_BLOCK_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_block.obj)
set(ARCHIAOS_KERNEL_GPT_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_gpt.obj)
set(ARCHIAOS_KERNEL_STORAGE_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_storage.obj)
set(ARCHIAOS_KERNEL_OPENFS_COMPAT_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_openfs_compat.obj)
set(ARCHIAOS_KERNEL_OPENFS_ADAPTER_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_openfs_adapter.obj)
set(ARCHIAOS_KERNEL_OPENFS_TEST_OBJ ${ARCHIAOS_BUILD}/archiaos_kernel_openfs_test.obj)
set(ARCHIAOS_OPENFS_C_SOURCES
    allocator.c bitmap.c crc32c.c dir.c extent.c file.c format.c fsck.c
    inode.c inode_alloc.c journal.c link.c mount.c path.c transaction.c)

set(ARCHIAOS_KERNEL_ELF ${ARCHIAOS_BUILD}/ARCHIAOS_kernel.elf)

set(ARCHIAOS_BOOT_MAIN_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_main.obj)
set(ARCHIAOS_BOOT_DEBUG_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_debug.obj)
set(ARCHIAOS_BOOT_FS_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_filesystem.obj)
set(ARCHIAOS_BOOT_ELF_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_elf_loader.obj)
set(ARCHIAOS_BOOT_FB_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_framebuffer.obj)
set(ARCHIAOS_BOOT_MM_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_memory_map.obj)
set(ARCHIAOS_BOOT_HANDOFF_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_handoff.obj)
set(ARCHIAOS_BOOT_PAGING_OBJ ${ARCHIAOS_BUILD}/archiaos_boot_paging.obj)
set(ARCHIAOS_BOOT_EFI ${ARCHIAOS_BUILD}/BOOTX64.EFI)

set(ARCHIAOS_ESP_DIR ${ARCHIAOS_BUILD}/esp)
set(ARCHIAOS_ESP_BOOT ${ARCHIAOS_ESP_DIR}/EFI/BOOT/BOOTX64.EFI)
set(ARCHIAOS_ESP_KERNEL ${ARCHIAOS_ESP_DIR}/EFI/ARCHIAOS/ARCHIAOS_kernel.elf)

function(archiaos_kernel_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${ARCHIAOS_CLANG}" -target x86_64-pc-none-elf
            -std=c++20 -ffreestanding -O2 -Wall -Wextra -Werror -fno-builtin -fno-exceptions -fno-rtti
            -fno-stack-protector -fno-pie -fno-asynchronous-unwind-tables
            -fno-threadsafe-statics -mgeneral-regs-only -mcmodel=kernel
            -mno-red-zone -mno-sse -mno-sse2
            -I"${ARCHIAOS_COMMON}" -I"${ARCHIAOS_CPU_DIR}" -I"${ARCHIAOS_MEMORY_DIR}"
            -c "${source}" -o "${output}"
        DEPENDS ${source} ${ARCHIAOS_COMMON}/boot_info.h
            ${ARCHIAOS_CPU_DIR}/cpuid.hpp
            ${ARCHIAOS_CPU_DIR}/features.hpp
            ${ARCHIAOS_CPU_DIR}/security.hpp
            ${ARCHIAOS_CPU_DIR}/idt.hpp
            ${ARCHIAOS_CPU_DIR}/gdt.hpp
            ${ARCHIAOS_CPU_DIR}/tss.hpp
            ${ARCHIAOS_CPU_DIR}/irq.hpp
            ${ARCHIAOS_CPU_DIR}/lapic.hpp
            ${ARCHIAOS_CPU_DIR}/ioapic.hpp
            ${ARCHIAOS_CPU_DIR}/acpi.hpp
            ${ARCHIAOS_CPU_DIR}/smp.hpp
            ${ARCHIAOS_CPU_DIR}/scheduler.hpp
            ${ARCHIAOS_MEMORY_DIR}/paging.hpp
            ${ARCHIAOS_MEMORY_DIR}/pmm.hpp
            ${ARCHIAOS_MEMORY_DIR}/vmm.hpp
            ${ARCHIAOS_MEMORY_DIR}/heap.hpp
            ${ARCHIAOS_MEMORY_DIR}/address_space.hpp
            ${ARCHIAOS_PROCESS_DIR}/process.hpp
    )
endfunction()

function(archiaos_kernel_asm_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${ARCHIAOS_CLANG}" -target x86_64-pc-none-elf
            -ffreestanding -O2 -fno-pie -fno-asynchronous-unwind-tables
            -mno-red-zone
            -c "${source}" -o "${output}"
        DEPENDS ${source}
    )
endfunction()

function(archiaos_openfs_c_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${ARCHIAOS_CLANG}" -target x86_64-pc-none-elf
            -x c -std=c11 -ffreestanding -O2 -Wall -Wextra -Werror -fno-builtin
            -fno-stack-protector -fno-pie -fno-asynchronous-unwind-tables
            -mno-red-zone -mno-sse -mno-sse2
            -I"${ARCHIAOS_OPENFS_COMPAT}" -I"${ARCHIAOS_OPENFS_INC}"
            -c "${source}" -o "${output}"
        DEPENDS ${source}
    )
endfunction()

function(archiaos_openfs_cpp_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${ARCHIAOS_CLANG}" -target x86_64-pc-none-elf
            -std=c++20 -ffreestanding -O2 -Wall -Wextra -Werror -fno-builtin
            -fno-exceptions -fno-rtti -fno-stack-protector -fno-pie
            -fno-asynchronous-unwind-tables -fno-threadsafe-statics
            -mgeneral-regs-only -mcmodel=kernel -mno-red-zone -mno-sse -mno-sse2
            -I"${ARCHIAOS_COMMON}" -I"${ARCHIAOS_CPU_DIR}" -I"${ARCHIAOS_MEMORY_DIR}"
            -I"${ARCHIAOS_OPENFS_INC}"
            -c "${source}" -o "${output}"
        DEPENDS ${source}
    )
endfunction()

archiaos_kernel_compile(${ARCHIAOS_KERNEL_OBJ} ${ARCHIAOS_KERNEL_DIR}/kernel.cpp)
archiaos_kernel_asm_compile(${ARCHIAOS_KERNEL_ENTRY_OBJ} ${ARCHIAOS_KERNEL_DIR}/entry.S)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_MEMORY_OBJ} ${ARCHIAOS_MEMORY_DIR}/memory.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_ACPI_OBJ} ${ARCHIAOS_CPU_DIR}/acpi.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_EXCEPTION_OBJ} ${ARCHIAOS_CPU_DIR}/exception.cpp)
archiaos_kernel_asm_compile(${ARCHIAOS_KERNEL_GDT_OBJ} ${ARCHIAOS_CPU_DIR}/gdt.S)
archiaos_kernel_asm_compile(${ARCHIAOS_KERNEL_IDT_OBJ} ${ARCHIAOS_CPU_DIR}/idt.S)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_IRQ_OBJ} ${ARCHIAOS_CPU_DIR}/irq.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_LAPIC_OBJ} ${ARCHIAOS_CPU_DIR}/lapic.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_IOAPIC_OBJ} ${ARCHIAOS_CPU_DIR}/ioapic.cpp)
archiaos_kernel_asm_compile(${ARCHIAOS_KERNEL_TSS_OBJ} ${ARCHIAOS_CPU_DIR}/tss.S)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_PAGING_OBJ} ${ARCHIAOS_MEMORY_DIR}/paging.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_PAGING_TEST_OBJ} ${ARCHIAOS_MEMORY_DIR}/tests/paging_test.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_PMM_OBJ} ${ARCHIAOS_MEMORY_DIR}/pmm.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_PMM_TEST_OBJ} ${ARCHIAOS_MEMORY_DIR}/tests/pmm_test.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_CPUID_OBJ} ${ARCHIAOS_CPU_DIR}/cpuid.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_CPU_REPORT_OBJ} ${ARCHIAOS_CPU_DIR}/cpu_report.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_SECURITY_OBJ} ${ARCHIAOS_CPU_DIR}/security.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_SMP_OBJ} ${ARCHIAOS_CPU_DIR}/smp.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_SCHEDULER_OBJ} ${ARCHIAOS_CPU_DIR}/scheduler.cpp)
archiaos_kernel_asm_compile(${ARCHIAOS_KERNEL_SMP_TRAMPOLINE_OBJ} ${ARCHIAOS_CPU_DIR}/smp_trampoline.S)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_VMM_OBJ} ${ARCHIAOS_MEMORY_DIR}/vmm.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_HEAP_OBJ} ${ARCHIAOS_MEMORY_DIR}/heap.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_ADDRESS_SPACE_OBJ} ${ARCHIAOS_MEMORY_DIR}/address_space.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_PROCESS_OBJ} ${ARCHIAOS_PROCESS_DIR}/process.cpp)
archiaos_kernel_asm_compile(${ARCHIAOS_KERNEL_RING3_OBJ} ${ARCHIAOS_PROCESS_DIR}/ring3.S)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_IPC_OBJ} ${ARCHIAOS_PROCESS_DIR}/ipc.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_VFS_OBJ} ${ARCHIAOS_FS_DIR}/vfs.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_PCI_OBJ} ${ARCHIAOS_DRIVERS_DIR}/pci.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_INPUT_OBJ} ${ARCHIAOS_DRIVERS_DIR}/input.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_GRAPHICS_OBJ} ${ARCHIAOS_DRIVERS_DIR}/graphics.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_GUI_OBJ} ${ARCHIAOS_DRIVERS_DIR}/gui.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_DEVICE_OBJ} ${ARCHIAOS_DRIVERS_DIR}/device.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_NET_OBJ} ${ARCHIAOS_DRIVERS_DIR}/net.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_BLOCK_OBJ} ${ARCHIAOS_DRIVERS_DIR}/block.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_GPT_OBJ} ${ARCHIAOS_FS_DIR}/gpt.cpp)
archiaos_kernel_compile(${ARCHIAOS_KERNEL_STORAGE_OBJ} ${ARCHIAOS_DRIVERS_DIR}/storage.cpp)

add_custom_command(
    OUTPUT ${ARCHIAOS_KERNEL_ELF}
    COMMAND "${ARCHIAOS_LLD}" -m elf_x86_64
        -T "${ARCHIAOS_KERNEL_DIR}/kernel.ld"
        "${ARCHIAOS_KERNEL_OBJ}"
        "${ARCHIAOS_KERNEL_ENTRY_OBJ}"
        "${ARCHIAOS_KERNEL_MEMORY_OBJ}"
        "${ARCHIAOS_KERNEL_ACPI_OBJ}"
        "${ARCHIAOS_KERNEL_EXCEPTION_OBJ}"
        "${ARCHIAOS_KERNEL_GDT_OBJ}"
        "${ARCHIAOS_KERNEL_IDT_OBJ}"
        "${ARCHIAOS_KERNEL_IRQ_OBJ}"
        "${ARCHIAOS_KERNEL_LAPIC_OBJ}"
        "${ARCHIAOS_KERNEL_IOAPIC_OBJ}"
        "${ARCHIAOS_KERNEL_TSS_OBJ}"
        "${ARCHIAOS_KERNEL_PAGING_OBJ}"
        "${ARCHIAOS_KERNEL_PAGING_TEST_OBJ}"
        "${ARCHIAOS_KERNEL_PMM_OBJ}"
        "${ARCHIAOS_KERNEL_PMM_TEST_OBJ}"
        "${ARCHIAOS_KERNEL_CPUID_OBJ}"
        "${ARCHIAOS_KERNEL_CPU_REPORT_OBJ}"
        "${ARCHIAOS_KERNEL_SECURITY_OBJ}"
        "${ARCHIAOS_KERNEL_SMP_OBJ}"
        "${ARCHIAOS_KERNEL_SCHEDULER_OBJ}"
        "${ARCHIAOS_KERNEL_SMP_TRAMPOLINE_OBJ}"
        "${ARCHIAOS_KERNEL_VMM_OBJ}" "${ARCHIAOS_KERNEL_HEAP_OBJ}"
        "${ARCHIAOS_KERNEL_ADDRESS_SPACE_OBJ}"
        "${ARCHIAOS_KERNEL_PROCESS_OBJ}" "${ARCHIAOS_KERNEL_RING3_OBJ}" "${ARCHIAOS_KERNEL_IPC_OBJ}" "${ARCHIAOS_KERNEL_VFS_OBJ}" "${ARCHIAOS_KERNEL_PCI_OBJ}" "${ARCHIAOS_KERNEL_INPUT_OBJ}" "${ARCHIAOS_KERNEL_GRAPHICS_OBJ}" "${ARCHIAOS_KERNEL_GUI_OBJ}" "${ARCHIAOS_KERNEL_DEVICE_OBJ}" "${ARCHIAOS_KERNEL_NET_OBJ}" "${ARCHIAOS_KERNEL_BLOCK_OBJ}" "${ARCHIAOS_KERNEL_GPT_OBJ}" "${ARCHIAOS_KERNEL_STORAGE_OBJ}"
        -o "${ARCHIAOS_KERNEL_ELF}"
    DEPENDS
        ${ARCHIAOS_KERNEL_OBJ} ${ARCHIAOS_KERNEL_ENTRY_OBJ}
        ${ARCHIAOS_KERNEL_MEMORY_OBJ} ${ARCHIAOS_KERNEL_ACPI_OBJ}
        ${ARCHIAOS_KERNEL_EXCEPTION_OBJ} ${ARCHIAOS_KERNEL_GDT_OBJ}
        ${ARCHIAOS_KERNEL_IDT_OBJ} ${ARCHIAOS_KERNEL_IRQ_OBJ}
        ${ARCHIAOS_KERNEL_LAPIC_OBJ} ${ARCHIAOS_KERNEL_IOAPIC_OBJ} ${ARCHIAOS_KERNEL_TSS_OBJ}
        ${ARCHIAOS_KERNEL_PAGING_OBJ} ${ARCHIAOS_KERNEL_PAGING_TEST_OBJ}
        ${ARCHIAOS_KERNEL_PMM_OBJ}
        ${ARCHIAOS_KERNEL_PMM_TEST_OBJ}
        ${ARCHIAOS_KERNEL_CPUID_OBJ} ${ARCHIAOS_KERNEL_CPU_REPORT_OBJ}
        ${ARCHIAOS_KERNEL_SECURITY_OBJ} ${ARCHIAOS_KERNEL_SMP_OBJ}
        ${ARCHIAOS_KERNEL_SCHEDULER_OBJ}
        ${ARCHIAOS_KERNEL_SMP_TRAMPOLINE_OBJ}
        ${ARCHIAOS_KERNEL_VMM_OBJ} ${ARCHIAOS_KERNEL_HEAP_OBJ}
        ${ARCHIAOS_KERNEL_ADDRESS_SPACE_OBJ}
        ${ARCHIAOS_KERNEL_PROCESS_OBJ} ${ARCHIAOS_KERNEL_RING3_OBJ} ${ARCHIAOS_KERNEL_IPC_OBJ} ${ARCHIAOS_KERNEL_VFS_OBJ} ${ARCHIAOS_KERNEL_PCI_OBJ} ${ARCHIAOS_KERNEL_INPUT_OBJ} ${ARCHIAOS_KERNEL_GRAPHICS_OBJ} ${ARCHIAOS_KERNEL_GUI_OBJ} ${ARCHIAOS_KERNEL_DEVICE_OBJ} ${ARCHIAOS_KERNEL_NET_OBJ} ${ARCHIAOS_KERNEL_BLOCK_OBJ} ${ARCHIAOS_KERNEL_GPT_OBJ} ${ARCHIAOS_KERNEL_STORAGE_OBJ}
        ${ARCHIAOS_KERNEL_DIR}/kernel.ld
        ${ARCHIAOS_KERNEL_OPENFS_COMPAT_OBJ} ${ARCHIAOS_KERNEL_OPENFS_ADAPTER_OBJ}
        ${ARCHIAOS_KERNEL_OPENFS_TEST_OBJ} ${ARCHIAOS_KERNEL_OPENFS_OBJECTS}
)

function(archiaos_boot_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${ARCHIAOS_CLANG}" -target x86_64-pc-win32-coff
            -std=c++20 -ffreestanding -fshort-wchar -fno-builtin
            -fno-exceptions -fno-rtti -fno-stack-protector -fno-pie -fno-asynchronous-unwind-tables
            -fno-threadsafe-statics -mgeneral-regs-only -Wall -Wextra -Werror
            -mno-red-zone -mno-sse -mno-sse2
            $<$<BOOL:${ARCHIAOS_BOOT_DEBUG}>:-DNOVOS_BOOT_DEBUG=1>
            -I"${ARCHIAOS_BOOT_INC}" -I"${ARCHIAOS_COMMON}"
            -c "${source}" -o "${output}"
        DEPENDS ${source} ${ARCHIAOS_BOOT_HEADERS}
    )
endfunction()

archiaos_boot_compile(${ARCHIAOS_BOOT_MAIN_OBJ} ${ARCHIAOS_BOOT_DIR}/main.cpp)
archiaos_boot_compile(${ARCHIAOS_BOOT_DEBUG_OBJ} ${ARCHIAOS_BOOT_DIR}/debug.cpp)
archiaos_boot_compile(${ARCHIAOS_BOOT_FS_OBJ} ${ARCHIAOS_BOOT_DIR}/filesystem.cpp)
archiaos_boot_compile(${ARCHIAOS_BOOT_ELF_OBJ} ${ARCHIAOS_BOOT_DIR}/elf_loader.cpp)
archiaos_boot_compile(${ARCHIAOS_BOOT_FB_OBJ} ${ARCHIAOS_BOOT_DIR}/framebuffer.cpp)
archiaos_boot_compile(${ARCHIAOS_BOOT_MM_OBJ} ${ARCHIAOS_BOOT_DIR}/memory_map.cpp)
archiaos_boot_compile(${ARCHIAOS_BOOT_PAGING_OBJ} ${ARCHIAOS_BOOT_DIR}/boot_paging.cpp)

add_custom_command(
    OUTPUT ${ARCHIAOS_BOOT_HANDOFF_OBJ}
    COMMAND "${ARCHIAOS_CLANG}" -target x86_64-pc-win32-coff
        -ffreestanding -c "${ARCHIAOS_BOOT_DIR}/handoff.S"
        -o "${ARCHIAOS_BOOT_HANDOFF_OBJ}"
    DEPENDS ${ARCHIAOS_BOOT_DIR}/handoff.S
)

add_custom_command(
    OUTPUT ${ARCHIAOS_BOOT_EFI}
    COMMAND "${ARCHIAOS_LLD_LINK}"
        /subsystem:efi_application
        /entry:efi_main
        /nodefaultlib
        /machine:x64
        "${ARCHIAOS_BOOT_MAIN_OBJ}"
        "${ARCHIAOS_BOOT_DEBUG_OBJ}"
        "${ARCHIAOS_BOOT_FS_OBJ}"
        "${ARCHIAOS_BOOT_ELF_OBJ}"
        "${ARCHIAOS_BOOT_FB_OBJ}"
        "${ARCHIAOS_BOOT_MM_OBJ}"
        "${ARCHIAOS_BOOT_HANDOFF_OBJ}"
        "${ARCHIAOS_BOOT_PAGING_OBJ}"
        /out:"${ARCHIAOS_BOOT_EFI}"
    DEPENDS
        ${ARCHIAOS_BOOT_MAIN_OBJ} ${ARCHIAOS_BOOT_DEBUG_OBJ}
        ${ARCHIAOS_BOOT_FS_OBJ} ${ARCHIAOS_BOOT_ELF_OBJ}
        ${ARCHIAOS_BOOT_FB_OBJ} ${ARCHIAOS_BOOT_MM_OBJ}
        ${ARCHIAOS_BOOT_HANDOFF_OBJ}
        ${ARCHIAOS_BOOT_PAGING_OBJ}
        ${ARCHIAOS_KERNEL_ELF}
)

add_custom_command(
    OUTPUT ${ARCHIAOS_ESP_BOOT}
    COMMAND ${CMAKE_COMMAND} -E make_directory "${ARCHIAOS_ESP_DIR}/EFI/BOOT"
    COMMAND ${CMAKE_COMMAND} -E make_directory "${ARCHIAOS_ESP_DIR}/EFI/ARCHIAOS"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${ARCHIAOS_BOOT_EFI}" "${ARCHIAOS_ESP_BOOT}"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${ARCHIAOS_KERNEL_ELF}" "${ARCHIAOS_ESP_KERNEL}"
    DEPENDS ${ARCHIAOS_BOOT_EFI} ${ARCHIAOS_KERNEL_ELF}
)

add_custom_target(archiaos_uefi_image ALL DEPENDS ${ARCHIAOS_ESP_BOOT})
add_custom_target(archiaos_uefi_artifacts
    DEPENDS ${ARCHIAOS_BOOT_EFI} ${ARCHIAOS_KERNEL_ELF} ${ARCHIAOS_ESP_BOOT})
