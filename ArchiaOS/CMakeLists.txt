cmake_minimum_required(VERSION 3.20)
project(ArchiaOS NONE)

set(OS_ROOT ${CMAKE_CURRENT_SOURCE_DIR})
set(OS_BUILD ${CMAKE_BINARY_DIR})

set(OS_CLANG "" CACHE FILEPATH "Clang++ compiler")
set(OS_LLD "" CACHE FILEPATH "LLD ELF linker")
set(OS_LLD_LINK "" CACHE FILEPATH "LLD PE/COFF linker")
option(OS_BOOT_DEBUG "Enable UEFI debug-port output" ON)

if(NOT OS_CLANG)
    find_program(OS_CLANG
        NAMES clang++.exe clang++ clang++-18 clang++-19 clang++-17
        PATHS /usr/bin /usr/local/bin)
endif()
if(NOT OS_LLD)
    find_program(OS_LLD
        NAMES ld.lld.exe ld.lld ld.lld-18 ld.lld-19 ld.lld-17
        PATHS /usr/bin /usr/local/bin)
endif()
if(NOT OS_LLD_LINK)
    find_program(OS_LLD_LINK
        NAMES lld-link.exe lld-link lld-link-18 lld-link-19 lld-link-17
        PATHS /usr/bin /usr/local/bin)
endif()

if(NOT OS_CLANG AND EXISTS "C:/Program Files/LLVM/bin/clang++.exe")
    set(OS_CLANG "C:/Program Files/LLVM/bin/clang++.exe")
endif()
if(NOT OS_LLD AND EXISTS "C:/Program Files/LLVM/bin/ld.lld.exe")
    set(OS_LLD "C:/Program Files/LLVM/bin/ld.lld.exe")
endif()
if(NOT OS_LLD_LINK AND EXISTS "C:/Program Files/LLVM/bin/lld-link.exe")
    set(OS_LLD_LINK "C:/Program Files/LLVM/bin/lld-link.exe")
endif()

foreach(tool IN ITEMS OS_CLANG OS_LLD OS_LLD_LINK)
    if(NOT ${tool})
        message(FATAL_ERROR "${tool} not found. Install LLVM or pass -D${tool}=...")
    endif()
endforeach()

set(OS_BOOT_DIR ${OS_ROOT}/boot/uefi/x86_64)
set(OS_BOOT_INC ${OS_ROOT}/boot/uefi/include)
set(OS_COMMON ${OS_ROOT}/common)
set(OS_KERNEL_DIR ${OS_ROOT}/kernel/x86_64/core)
set(OS_CPU_DIR ${OS_ROOT}/kernel/x86_64/cpu)
set(OS_MEMORY_DIR ${OS_ROOT}/kernel/x86_64/memory)
set(OS_PROCESS_DIR ${OS_ROOT}/kernel/x86_64/process)
set(OS_FS_DIR ${OS_ROOT}/kernel/x86_64/fs)
set(OS_DRIVERS_DIR ${OS_ROOT}/kernel/x86_64/drivers)
set(OS_OPENFS_ROOT ${OS_ROOT}/kernel/fs/openfs)
set(OS_OPENFS_INC ${OS_OPENFS_ROOT}/include)
set(OS_OPENFS_COMPAT ${OS_FS_DIR}/openfs_compat)

set(OS_BOOT_HEADERS
    ${OS_BOOT_INC}/uefi.h
    ${OS_BOOT_INC}/boot_debug.h
    ${OS_BOOT_INC}/filesystem.h
    ${OS_BOOT_INC}/elf_loader.h
    ${OS_BOOT_INC}/framebuffer.h
    ${OS_BOOT_INC}/memory_map.h
    ${OS_BOOT_INC}/handoff.h
    ${OS_BOOT_INC}/boot_paging.h
    ${OS_COMMON}/boot_info.h
)

set(OS_KERNEL_OBJ ${OS_BUILD}/os_kernel.obj)
set(OS_KERNEL_ENTRY_OBJ ${OS_BUILD}/os_kernel_entry.obj)
set(OS_KERNEL_MEMORY_OBJ ${OS_BUILD}/os_kernel_memory.obj)
set(OS_KERNEL_ACPI_OBJ ${OS_BUILD}/os_kernel_acpi.obj)
set(OS_KERNEL_EXCEPTION_OBJ ${OS_BUILD}/os_kernel_exception.obj)
set(OS_KERNEL_GDT_OBJ ${OS_BUILD}/os_kernel_gdt.obj)
set(OS_KERNEL_IDT_OBJ ${OS_BUILD}/os_kernel_idt.obj)
set(OS_KERNEL_IRQ_OBJ ${OS_BUILD}/os_kernel_irq.obj)
set(OS_KERNEL_LAPIC_OBJ ${OS_BUILD}/os_kernel_lapic.obj)
set(OS_KERNEL_IOAPIC_OBJ ${OS_BUILD}/os_kernel_ioapic.obj)
set(OS_KERNEL_TSS_OBJ ${OS_BUILD}/os_kernel_tss.obj)
set(OS_KERNEL_PAGING_OBJ ${OS_BUILD}/os_kernel_paging.obj)
set(OS_KERNEL_PAGING_TEST_OBJ ${OS_BUILD}/os_kernel_paging_test.obj)
set(OS_KERNEL_PMM_OBJ ${OS_BUILD}/os_kernel_pmm.obj)
set(OS_KERNEL_PMM_TEST_OBJ ${OS_BUILD}/os_kernel_pmm_test.obj)
set(OS_KERNEL_CPUID_OBJ ${OS_BUILD}/os_kernel_cpuid.obj)
set(OS_KERNEL_CPU_REPORT_OBJ ${OS_BUILD}/os_kernel_cpu_report.obj)
set(OS_KERNEL_SECURITY_OBJ ${OS_BUILD}/os_kernel_security.obj)
set(OS_KERNEL_SMP_OBJ ${OS_BUILD}/os_kernel_smp.obj)
set(OS_KERNEL_SCHEDULER_OBJ ${OS_BUILD}/os_kernel_scheduler.obj)
set(OS_KERNEL_SMP_TRAMPOLINE_OBJ ${OS_BUILD}/os_kernel_smp_trampoline.obj)
set(OS_KERNEL_VMM_OBJ ${OS_BUILD}/os_kernel_vmm.obj)
set(OS_KERNEL_HEAP_OBJ ${OS_BUILD}/os_kernel_heap.obj)
set(OS_KERNEL_ADDRESS_SPACE_OBJ ${OS_BUILD}/os_kernel_address_space.obj)
set(OS_KERNEL_PROCESS_OBJ ${OS_BUILD}/os_kernel_process.obj)
set(OS_KERNEL_RING3_OBJ ${OS_BUILD}/os_kernel_ring3.obj)
set(OS_KERNEL_IPC_OBJ ${OS_BUILD}/os_kernel_ipc.obj)
set(OS_KERNEL_VFS_OBJ ${OS_BUILD}/os_kernel_vfs.obj)
set(OS_KERNEL_PCI_OBJ ${OS_BUILD}/os_kernel_pci.obj)
set(OS_KERNEL_INPUT_OBJ ${OS_BUILD}/os_kernel_input.obj)
set(OS_KERNEL_GRAPHICS_OBJ ${OS_BUILD}/os_kernel_graphics.obj)
set(OS_KERNEL_GUI_OBJ ${OS_BUILD}/os_kernel_gui.obj)
set(OS_KERNEL_DEVICE_OBJ ${OS_BUILD}/os_kernel_device.obj)
set(OS_KERNEL_NET_OBJ ${OS_BUILD}/os_kernel_net.obj)
set(OS_KERNEL_BLOCK_OBJ ${OS_BUILD}/os_kernel_block.obj)
set(OS_KERNEL_GPT_OBJ ${OS_BUILD}/os_kernel_gpt.obj)
set(OS_KERNEL_STORAGE_OBJ ${OS_BUILD}/os_kernel_storage.obj)
set(OS_KERNEL_OPENFS_COMPAT_OBJ ${OS_BUILD}/os_kernel_openfs_compat.obj)
set(OS_KERNEL_OPENFS_ADAPTER_OBJ ${OS_BUILD}/os_kernel_openfs_adapter.obj)
set(OS_KERNEL_OPENFS_TEST_OBJ ${OS_BUILD}/os_kernel_openfs_test.obj)
set(OS_OPENFS_C_SOURCES
    allocator.c bitmap.c crc32c.c dir.c extent.c file.c format.c fsck.c
    inode.c inode_alloc.c journal.c link.c mount.c path.c transaction.c)

set(OS_KERNEL_ELF ${OS_BUILD}/OS_kernel.elf)

set(OS_BOOT_MAIN_OBJ ${OS_BUILD}/os_boot_main.obj)
set(OS_BOOT_DEBUG_OBJ ${OS_BUILD}/os_boot_debug.obj)
set(OS_BOOT_FS_OBJ ${OS_BUILD}/os_boot_filesystem.obj)
set(OS_BOOT_ELF_OBJ ${OS_BUILD}/os_boot_elf_loader.obj)
set(OS_BOOT_FB_OBJ ${OS_BUILD}/os_boot_framebuffer.obj)
set(OS_BOOT_MM_OBJ ${OS_BUILD}/os_boot_memory_map.obj)
set(OS_BOOT_HANDOFF_OBJ ${OS_BUILD}/os_boot_handoff.obj)
set(OS_BOOT_PAGING_OBJ ${OS_BUILD}/os_boot_paging.obj)
set(OS_BOOT_EFI ${OS_BUILD}/BOOTX64.EFI)

set(OS_ESP_DIR ${OS_BUILD}/esp)
set(OS_ESP_BOOT ${OS_ESP_DIR}/EFI/BOOT/BOOTX64.EFI)
set(OS_ESP_KERNEL ${OS_ESP_DIR}/EFI/OS/OS_kernel.elf)

function(os_kernel_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${OS_CLANG}" -target x86_64-pc-none-elf
            -std=c++20 -ffreestanding -O2 -Wall -Wextra -Werror -fno-builtin -fno-exceptions -fno-rtti
            -fno-stack-protector -fno-pie -fno-asynchronous-unwind-tables
            -fno-threadsafe-statics -mgeneral-regs-only -mcmodel=kernel
            -mno-red-zone -mno-sse -mno-sse2
            -I"${OS_COMMON}" -I"${OS_CPU_DIR}" -I"${OS_MEMORY_DIR}" -I"${OS_OPENFS_INC}"
            -c "${source}" -o "${output}"
        DEPENDS ${source} ${OS_COMMON}/boot_info.h
            ${OS_CPU_DIR}/cpuid.hpp
            ${OS_CPU_DIR}/features.hpp
            ${OS_CPU_DIR}/security.hpp
            ${OS_CPU_DIR}/idt.hpp
            ${OS_CPU_DIR}/gdt.hpp
            ${OS_CPU_DIR}/tss.hpp
            ${OS_CPU_DIR}/irq.hpp
            ${OS_CPU_DIR}/lapic.hpp
            ${OS_CPU_DIR}/ioapic.hpp
            ${OS_CPU_DIR}/acpi.hpp
            ${OS_CPU_DIR}/smp.hpp
            ${OS_CPU_DIR}/scheduler.hpp
            ${OS_MEMORY_DIR}/paging.hpp
            ${OS_MEMORY_DIR}/pmm.hpp
            ${OS_MEMORY_DIR}/vmm.hpp
            ${OS_MEMORY_DIR}/heap.hpp
            ${OS_MEMORY_DIR}/address_space.hpp
            ${OS_PROCESS_DIR}/process.hpp
    )
endfunction()

function(os_kernel_asm_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${OS_CLANG}" -target x86_64-pc-none-elf
            -ffreestanding -O2 -fno-pie -fno-asynchronous-unwind-tables
            -mno-red-zone
            -c "${source}" -o "${output}"
        DEPENDS ${source}
    )
endfunction()

function(os_openfs_c_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${OS_CLANG}" -target x86_64-pc-none-elf
            -x c -std=c11 -ffreestanding -O2 -Wall -Wextra -Werror -Wno-logical-op-parentheses -fno-builtin
            -fno-stack-protector -fno-pie -fno-asynchronous-unwind-tables
            -mgeneral-regs-only -mcmodel=kernel -mno-red-zone -mno-sse -mno-sse2
            -I"${OS_OPENFS_COMPAT}" -I"${OS_OPENFS_INC}"
            -c "${source}" -o "${output}"
        DEPENDS ${source}
    )
endfunction()

function(os_openfs_cpp_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${OS_CLANG}" -target x86_64-pc-none-elf
            -std=c++20 -ffreestanding -O2 -Wall -Wextra -Werror -fno-builtin
            -fno-exceptions -fno-rtti -fno-stack-protector -fno-pie
            -fno-asynchronous-unwind-tables -fno-threadsafe-statics
            -mgeneral-regs-only -mcmodel=kernel -mno-red-zone -mno-sse -mno-sse2
            -I"${OS_COMMON}" -I"${OS_CPU_DIR}" -I"${OS_MEMORY_DIR}"
            -I"${OS_OPENFS_INC}"
            -c "${source}" -o "${output}"
        DEPENDS ${source}
    )
endfunction()

os_kernel_compile(${OS_KERNEL_OBJ} ${OS_KERNEL_DIR}/kernel.cpp)
os_kernel_asm_compile(${OS_KERNEL_ENTRY_OBJ} ${OS_KERNEL_DIR}/entry.S)
os_kernel_compile(${OS_KERNEL_MEMORY_OBJ} ${OS_MEMORY_DIR}/memory.cpp)
os_kernel_compile(${OS_KERNEL_ACPI_OBJ} ${OS_CPU_DIR}/acpi.cpp)
os_kernel_compile(${OS_KERNEL_EXCEPTION_OBJ} ${OS_CPU_DIR}/exception.cpp)
os_kernel_asm_compile(${OS_KERNEL_GDT_OBJ} ${OS_CPU_DIR}/gdt.S)
os_kernel_asm_compile(${OS_KERNEL_IDT_OBJ} ${OS_CPU_DIR}/idt.S)
os_kernel_compile(${OS_KERNEL_IRQ_OBJ} ${OS_CPU_DIR}/irq.cpp)
os_kernel_compile(${OS_KERNEL_LAPIC_OBJ} ${OS_CPU_DIR}/lapic.cpp)
os_kernel_compile(${OS_KERNEL_IOAPIC_OBJ} ${OS_CPU_DIR}/ioapic.cpp)
os_kernel_asm_compile(${OS_KERNEL_TSS_OBJ} ${OS_CPU_DIR}/tss.S)
os_kernel_compile(${OS_KERNEL_PAGING_OBJ} ${OS_MEMORY_DIR}/paging.cpp)
os_kernel_compile(${OS_KERNEL_PAGING_TEST_OBJ} ${OS_MEMORY_DIR}/tests/paging_test.cpp)
os_kernel_compile(${OS_KERNEL_PMM_OBJ} ${OS_MEMORY_DIR}/pmm.cpp)
os_kernel_compile(${OS_KERNEL_PMM_TEST_OBJ} ${OS_MEMORY_DIR}/tests/pmm_test.cpp)
os_kernel_compile(${OS_KERNEL_CPUID_OBJ} ${OS_CPU_DIR}/cpuid.cpp)
os_kernel_compile(${OS_KERNEL_CPU_REPORT_OBJ} ${OS_CPU_DIR}/cpu_report.cpp)
os_kernel_compile(${OS_KERNEL_SECURITY_OBJ} ${OS_CPU_DIR}/security.cpp)
os_kernel_compile(${OS_KERNEL_SMP_OBJ} ${OS_CPU_DIR}/smp.cpp)
os_kernel_compile(${OS_KERNEL_SCHEDULER_OBJ} ${OS_CPU_DIR}/scheduler.cpp)
os_kernel_asm_compile(${OS_KERNEL_SMP_TRAMPOLINE_OBJ} ${OS_CPU_DIR}/smp_trampoline.S)
os_kernel_compile(${OS_KERNEL_VMM_OBJ} ${OS_MEMORY_DIR}/vmm.cpp)
os_kernel_compile(${OS_KERNEL_HEAP_OBJ} ${OS_MEMORY_DIR}/heap.cpp)
os_kernel_compile(${OS_KERNEL_ADDRESS_SPACE_OBJ} ${OS_MEMORY_DIR}/address_space.cpp)
os_kernel_compile(${OS_KERNEL_PROCESS_OBJ} ${OS_PROCESS_DIR}/process.cpp)
os_kernel_asm_compile(${OS_KERNEL_RING3_OBJ} ${OS_PROCESS_DIR}/ring3.S)
os_kernel_compile(${OS_KERNEL_IPC_OBJ} ${OS_PROCESS_DIR}/ipc.cpp)
os_kernel_compile(${OS_KERNEL_VFS_OBJ} ${OS_FS_DIR}/vfs.cpp)
os_kernel_compile(${OS_KERNEL_PCI_OBJ} ${OS_DRIVERS_DIR}/pci.cpp)
os_kernel_compile(${OS_KERNEL_INPUT_OBJ} ${OS_DRIVERS_DIR}/input.cpp)
os_kernel_compile(${OS_KERNEL_GRAPHICS_OBJ} ${OS_DRIVERS_DIR}/graphics.cpp)
os_kernel_compile(${OS_KERNEL_GUI_OBJ} ${OS_DRIVERS_DIR}/gui.cpp)
os_kernel_compile(${OS_KERNEL_DEVICE_OBJ} ${OS_DRIVERS_DIR}/device.cpp)
os_kernel_compile(${OS_KERNEL_NET_OBJ} ${OS_DRIVERS_DIR}/net.cpp)
os_kernel_compile(${OS_KERNEL_BLOCK_OBJ} ${OS_DRIVERS_DIR}/block.cpp)
os_kernel_compile(${OS_KERNEL_GPT_OBJ} ${OS_FS_DIR}/gpt.cpp)
os_kernel_compile(${OS_KERNEL_STORAGE_OBJ} ${OS_DRIVERS_DIR}/storage.cpp)

os_openfs_c_compile(${OS_KERNEL_OPENFS_COMPAT_OBJ} ${OS_OPENFS_COMPAT}/openfs_kernel_compat.c)
foreach(openfs_source IN LISTS OS_OPENFS_C_SOURCES)
    get_filename_component(openfs_name ${openfs_source} NAME_WE)
    set(openfs_object ${OS_BUILD}/os_openfs_${openfs_name}.obj)
    os_openfs_c_compile(${openfs_object} ${OS_OPENFS_ROOT}/src/${openfs_source})
    list(APPEND OS_KERNEL_OPENFS_OBJECTS ${openfs_object})
endforeach()
os_openfs_cpp_compile(${OS_KERNEL_OPENFS_ADAPTER_OBJ} ${OS_FS_DIR}/openfs_adapter.cpp)
os_openfs_cpp_compile(${OS_KERNEL_OPENFS_TEST_OBJ} ${OS_FS_DIR}/openfs_test.cpp)

add_custom_command(
    OUTPUT ${OS_KERNEL_ELF}
    COMMAND "${OS_LLD}" -m elf_x86_64
        -T "${OS_KERNEL_DIR}/kernel.ld"
        "${OS_KERNEL_OBJ}"
        "${OS_KERNEL_ENTRY_OBJ}"
        "${OS_KERNEL_MEMORY_OBJ}"
        "${OS_KERNEL_ACPI_OBJ}"
        "${OS_KERNEL_EXCEPTION_OBJ}"
        "${OS_KERNEL_GDT_OBJ}"
        "${OS_KERNEL_IDT_OBJ}"
        "${OS_KERNEL_IRQ_OBJ}"
        "${OS_KERNEL_LAPIC_OBJ}"
        "${OS_KERNEL_IOAPIC_OBJ}"
        "${OS_KERNEL_TSS_OBJ}"
        "${OS_KERNEL_PAGING_OBJ}"
        "${OS_KERNEL_PAGING_TEST_OBJ}"
        "${OS_KERNEL_PMM_OBJ}"
        "${OS_KERNEL_PMM_TEST_OBJ}"
        "${OS_KERNEL_CPUID_OBJ}"
        "${OS_KERNEL_CPU_REPORT_OBJ}"
        "${OS_KERNEL_SECURITY_OBJ}"
        "${OS_KERNEL_SMP_OBJ}"
        "${OS_KERNEL_SCHEDULER_OBJ}"
        "${OS_KERNEL_SMP_TRAMPOLINE_OBJ}"
        "${OS_KERNEL_VMM_OBJ}" "${OS_KERNEL_HEAP_OBJ}"
        "${OS_KERNEL_ADDRESS_SPACE_OBJ}"
        "${OS_KERNEL_PROCESS_OBJ}" "${OS_KERNEL_RING3_OBJ}" "${OS_KERNEL_IPC_OBJ}" "${OS_KERNEL_VFS_OBJ}" "${OS_KERNEL_PCI_OBJ}" "${OS_KERNEL_INPUT_OBJ}" "${OS_KERNEL_GRAPHICS_OBJ}" "${OS_KERNEL_GUI_OBJ}" "${OS_KERNEL_DEVICE_OBJ}" "${OS_KERNEL_NET_OBJ}" "${OS_KERNEL_BLOCK_OBJ}" "${OS_KERNEL_GPT_OBJ}" "${OS_KERNEL_STORAGE_OBJ}"
        "${OS_KERNEL_OPENFS_COMPAT_OBJ}" "${OS_KERNEL_OPENFS_ADAPTER_OBJ}" "${OS_KERNEL_OPENFS_TEST_OBJ}"
        ${OS_KERNEL_OPENFS_OBJECTS}
        -o "${OS_KERNEL_ELF}"
    DEPENDS
        ${OS_KERNEL_OBJ} ${OS_KERNEL_ENTRY_OBJ}
        ${OS_KERNEL_MEMORY_OBJ} ${OS_KERNEL_ACPI_OBJ}
        ${OS_KERNEL_EXCEPTION_OBJ} ${OS_KERNEL_GDT_OBJ}
        ${OS_KERNEL_IDT_OBJ} ${OS_KERNEL_IRQ_OBJ}
        ${OS_KERNEL_LAPIC_OBJ} ${OS_KERNEL_IOAPIC_OBJ} ${OS_KERNEL_TSS_OBJ}
        ${OS_KERNEL_PAGING_OBJ} ${OS_KERNEL_PAGING_TEST_OBJ}
        ${OS_KERNEL_PMM_OBJ}
        ${OS_KERNEL_PMM_TEST_OBJ}
        ${OS_KERNEL_CPUID_OBJ} ${OS_KERNEL_CPU_REPORT_OBJ}
        ${OS_KERNEL_SECURITY_OBJ} ${OS_KERNEL_SMP_OBJ}
        ${OS_KERNEL_SCHEDULER_OBJ}
        ${OS_KERNEL_SMP_TRAMPOLINE_OBJ}
        ${OS_KERNEL_VMM_OBJ} ${OS_KERNEL_HEAP_OBJ}
        ${OS_KERNEL_ADDRESS_SPACE_OBJ}
        ${OS_KERNEL_PROCESS_OBJ} ${OS_KERNEL_RING3_OBJ} ${OS_KERNEL_IPC_OBJ} ${OS_KERNEL_VFS_OBJ} ${OS_KERNEL_PCI_OBJ} ${OS_KERNEL_INPUT_OBJ} ${OS_KERNEL_GRAPHICS_OBJ} ${OS_KERNEL_GUI_OBJ} ${OS_KERNEL_DEVICE_OBJ} ${OS_KERNEL_NET_OBJ} ${OS_KERNEL_BLOCK_OBJ} ${OS_KERNEL_GPT_OBJ} ${OS_KERNEL_STORAGE_OBJ}
        ${OS_KERNEL_DIR}/kernel.ld
        ${OS_KERNEL_OPENFS_COMPAT_OBJ} ${OS_KERNEL_OPENFS_ADAPTER_OBJ}
        ${OS_KERNEL_OPENFS_TEST_OBJ} ${OS_KERNEL_OPENFS_OBJECTS}
)

function(os_boot_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${OS_CLANG}" -target x86_64-pc-win32-coff
            -std=c++20 -ffreestanding -fshort-wchar -fno-builtin
            -fno-exceptions -fno-rtti -fno-stack-protector -fno-pie -fno-asynchronous-unwind-tables
            -fno-threadsafe-statics -mgeneral-regs-only -Wall -Wextra -Werror
            -mno-red-zone -mno-sse -mno-sse2
            $<$<BOOL:${OS_BOOT_DEBUG}>:-DBOOT_DEBUG=1>
            -I"${OS_BOOT_INC}" -I"${OS_COMMON}"
            -c "${source}" -o "${output}"
        DEPENDS ${source} ${OS_BOOT_HEADERS}
    )
endfunction()

os_boot_compile(${OS_BOOT_MAIN_OBJ} ${OS_BOOT_DIR}/main.cpp)
os_boot_compile(${OS_BOOT_DEBUG_OBJ} ${OS_BOOT_DIR}/debug.cpp)
os_boot_compile(${OS_BOOT_FS_OBJ} ${OS_BOOT_DIR}/filesystem.cpp)
os_boot_compile(${OS_BOOT_ELF_OBJ} ${OS_BOOT_DIR}/elf_loader.cpp)
os_boot_compile(${OS_BOOT_FB_OBJ} ${OS_BOOT_DIR}/framebuffer.cpp)
os_boot_compile(${OS_BOOT_MM_OBJ} ${OS_BOOT_DIR}/memory_map.cpp)
os_boot_compile(${OS_BOOT_PAGING_OBJ} ${OS_BOOT_DIR}/boot_paging.cpp)

add_custom_command(
    OUTPUT ${OS_BOOT_HANDOFF_OBJ}
    COMMAND "${OS_CLANG}" -target x86_64-pc-win32-coff
        -ffreestanding -c "${OS_BOOT_DIR}/handoff.S"
        -o "${OS_BOOT_HANDOFF_OBJ}"
    DEPENDS ${OS_BOOT_DIR}/handoff.S
)

add_custom_command(
    OUTPUT ${OS_BOOT_EFI}
    COMMAND "${OS_LLD_LINK}"
        /subsystem:efi_application
        /entry:efi_main
        /nodefaultlib
        /machine:x64
        "${OS_BOOT_MAIN_OBJ}"
        "${OS_BOOT_DEBUG_OBJ}"
        "${OS_BOOT_FS_OBJ}"
        "${OS_BOOT_ELF_OBJ}"
        "${OS_BOOT_FB_OBJ}"
        "${OS_BOOT_MM_OBJ}"
        "${OS_BOOT_HANDOFF_OBJ}"
        "${OS_BOOT_PAGING_OBJ}"
        /out:"${OS_BOOT_EFI}"
    DEPENDS
        ${OS_BOOT_MAIN_OBJ} ${OS_BOOT_DEBUG_OBJ}
        ${OS_BOOT_FS_OBJ} ${OS_BOOT_ELF_OBJ}
        ${OS_BOOT_FB_OBJ} ${OS_BOOT_MM_OBJ}
        ${OS_BOOT_HANDOFF_OBJ}
        ${OS_BOOT_PAGING_OBJ}
        ${OS_KERNEL_ELF}
)

add_custom_command(
    OUTPUT ${OS_ESP_BOOT}
    COMMAND ${CMAKE_COMMAND} -E make_directory "${OS_ESP_DIR}/EFI/BOOT"
    COMMAND ${CMAKE_COMMAND} -E make_directory "${OS_ESP_DIR}/EFI/OS"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${OS_BOOT_EFI}" "${OS_ESP_BOOT}"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${OS_KERNEL_ELF}" "${OS_ESP_KERNEL}"
    DEPENDS ${OS_BOOT_EFI} ${OS_KERNEL_ELF}
)

add_custom_target(os_uefi_image ALL DEPENDS ${OS_ESP_BOOT})
add_custom_target(os_uefi_artifacts
    DEPENDS ${OS_BOOT_EFI} ${OS_KERNEL_ELF} ${OS_ESP_BOOT})
