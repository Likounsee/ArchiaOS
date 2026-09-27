cmake_minimum_required(VERSION 4.0)
project(NovOS NONE)

set(NOVOS_ROOT ${CMAKE_CURRENT_SOURCE_DIR})
set(NOVOS_BUILD ${CMAKE_BINARY_DIR})
set(NOVOS_CLANG "C:/Program Files/LLVM/bin/clang++.exe" CACHE FILEPATH "Clang++")
set(NOVOS_LLD "C:/Program Files/LLVM/bin/ld.lld.exe" CACHE FILEPATH "LLD")
set(NOVOS_LLD_LINK "C:/Program Files/LLVM/bin/lld-link.exe" CACHE FILEPATH "lld-link")

if(NOT EXISTS "${NOVOS_CLANG}")
    message(FATAL_ERROR "Clang++ not found: ${NOVOS_CLANG}")
endif()
if(NOT EXISTS "${NOVOS_LLD}")
    message(FATAL_ERROR "ld.lld not found: ${NOVOS_LLD}")
endif()
if(NOT EXISTS "${NOVOS_LLD_LINK}")
    message(FATAL_ERROR "lld-link not found: ${NOVOS_LLD_LINK}")
endif()

set(NOVOS_BOOT_SOURCES
    ${NOVOS_ROOT}/boot/uefi/x86_64/main.cpp
    ${NOVOS_ROOT}/boot/uefi/x86_64/debug.cpp
    ${NOVOS_ROOT}/boot/uefi/x86_64/filesystem.cpp
    ${NOVOS_ROOT}/boot/uefi/x86_64/elf_loader.cpp
    ${NOVOS_ROOT}/boot/uefi/x86_64/framebuffer.cpp
    ${NOVOS_ROOT}/boot/uefi/x86_64/memory_map.cpp
    ${NOVOS_ROOT}/boot/uefi/x86_64/handoff.S
)

set(NOVOS_KERNEL_SOURCES
    ${NOVOS_ROOT}/kernel/x86_64/core/kernel.cpp
    ${NOVOS_ROOT}/kernel/x86_64/core/entry.S
)

set(NOVOS_BOOT_OBJ ${NOVOS_BUILD}/novos_boot.obj)
set(NOVOS_KERNEL_OBJ ${NOVOS_BUILD}/novos_kernel.obj)
set(NOVOS_KERNEL_ENTRY_OBJ ${NOVOS_BUILD}/novos_kernel_entry.obj)
set(NOVOS_KERNEL_ELF ${NOVOS_BUILD}/NOVOS_kernel.elf)
set(NOVOS_BOOT_EFI ${NOVOS_BUILD}/BOOTX64.EFI)
set(NOVOS_ESP ${NOVOS_BUILD}/esp)

add_custom_command(
    OUTPUT ${NOVOS_KERNEL_OBJ}
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-none-elf -ffreestanding -fno-exceptions -fno-rtti -fno-stack-protector -mno-red-zone -I"${NOVOS_ROOT}/common" -c "${NOVOS_ROOT}/kernel/x86_64/core/kernel.cpp" -o "${NOVOS_KERNEL_OBJ}"
    DEPENDS ${NOVOS_ROOT}/kernel/x86_64/core/kernel.cpp ${NOVOS_ROOT}/common/boot_info.h
)

add_custom_command(
    OUTPUT ${NOVOS_KERNEL_ENTRY_OBJ}
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-none-elf -ffreestanding -c "${NOVOS_ROOT}/kernel/x86_64/core/entry.S" -o "${NOVOS_KERNEL_ENTRY_OBJ}"
    DEPENDS ${NOVOS_ROOT}/kernel/x86_64/core/entry.S
)

add_custom_command(
    OUTPUT ${NOVOS_KERNEL_ELF}
    COMMAND "${NOVOS_LLD}" -m elf_x86_64 -T "${NOVOS_ROOT}/kernel/x86_64/core/kernel.ld" "${NOVOS_KERNEL_OBJ}" "${NOVOS_KERNEL_ENTRY_OBJ}" -o "${NOVOS_KERNEL_ELF}"
    DEPENDS ${NOVOS_KERNEL_OBJ} ${NOVOS_KERNEL_ENTRY_OBJ} ${NOVOS_ROOT}/kernel/x86_64/core/kernel.ld
)

add_custom_command(
    OUTPUT ${NOVOS_BOOT_OBJ}
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff -ffreestanding -fshort-wchar -fno-exceptions -fno-rtti -fno-stack-protector -mno-red-zone -DNOVOS_BOOT_DEBUG=1 -I"${NOVOS_ROOT}/boot/uefi/include" -I"${NOVOS_ROOT}/common" -c "${NOVOS_ROOT}/boot/uefi/x86_64/main.cpp" -o "${NOVOS_BOOT_OBJ}"
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff -ffreestanding -fshort-wchar -fno-exceptions -fno-rtti -fno-stack-protector -mno-red-zone -DNOVOS_BOOT_DEBUG=1 -I"${NOVOS_ROOT}/boot/uefi/include" -I"${NOVOS_ROOT}/common" -c "${NOVOS_ROOT}/boot/uefi/x86_64/debug.cpp" -o "${NOVOS_BUILD}/novos_debug.obj"
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff -ffreestanding -fshort-wchar -fno-exceptions -fno-rtti -fno-stack-protector -mno-red-zone -DNOVOS_BOOT_DEBUG=1 -I"${NOVOS_ROOT}/boot/uefi/include" -I"${NOVOS_ROOT}/common" -c "${NOVOS_ROOT}/boot/uefi/x86_64/filesystem.cpp" -o "${NOVOS_BUILD}/novos_filesystem.obj"
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff -ffreestanding -fshort-wchar -fno-exceptions -fno-rtti -fno-stack-protector -mno-red-zone -DNOVOS_BOOT_DEBUG=1 -I"${NOVOS_ROOT}/boot/uefi/include" -I"${NOVOS_ROOT}/common" -c "${NOVOS_ROOT}/boot/uefi/x86_64/elf_loader.cpp" -o "${NOVOS_BUILD}/novos_elf_loader.obj"
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff -ffreestanding -fshort-wchar -fno-exceptions -fno-rtti -fno-stack-protector -mno-red-zone -DNOVOS_BOOT_DEBUG=1 -I"${NOVOS_ROOT}/boot/uefi/include" -I"${NOVOS_ROOT}/common" -c "${NOVOS_ROOT}/boot/uefi/x86_64/framebuffer.cpp" -o "${NOVOS_BUILD}/novos_framebuffer.obj"
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff -ffreestanding -fshort-wchar -fno-exceptions -fno-rtti -fno-stack-protector -mno-red-zone -DNOVOS_BOOT_DEBUG=1 -I"${NOVOS_ROOT}/boot/uefi/include" -I"${NOVOS_ROOT}/common" -c "${NOVOS_ROOT}/boot/uefi/x86_64/memory_map.cpp" -o "${NOVOS_BUILD}/novos_memory_map.obj"
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff -ffreestanding -c "${NOVOS_ROOT}/boot/uefi/x86_64/handoff.S" -o "${NOVOS_BUILD}/novos_handoff.obj"
    DEPENDS ${NOVOS_BOOT_SOURCES} ${NOVOS_ROOT}/common/boot_info.h ${NOVOS_ROOT}/boot/uefi/include/uefi.h
)

add_custom_command(
    OUTPUT ${NOVOS_BOOT_EFI}
    COMMAND "${NOVOS_LLD_LINK}" /subsystem:efi_application /entry:efi_main /nodefaultlib /machine:x64
        "${NOVOS_BOOT_OBJ}" "${NOVOS_BUILD}/novos_debug.obj" "${NOVOS_BUILD}/novos_filesystem.obj"
        "${NOVOS_BUILD}/novos_elf_loader.obj" "${NOVOS_BUILD}/novos_framebuffer.obj"
        "${NOVOS_BUILD}/novos_memory_map.obj" "${NOVOS_BUILD}/novos_handoff.obj"
        /out:"${NOVOS_BOOT_EFI}"
    DEPENDS ${NOVOS_BOOT_OBJ} ${NOVOS_KERNEL_ELF}
)

add_custom_command(
    OUTPUT ${NOVOS_ESP}/EFI/BOOT/BOOTX64.EFI
    COMMAND ${CMAKE_COMMAND} -E make_directory "${NOVOS_ESP}/EFI/BOOT"
    COMMAND ${CMAKE_COMMAND} -E make_directory "${NOVOS_ESP}/EFI/NOVOS"
    COMMAND ${CMAKE_COMMAND} -E copy "${NOVOS_BOOT_EFI}" "${NOVOS_ESP}/EFI/BOOT/BOOTX64.EFI"
    COMMAND ${CMAKE_COMMAND} -E copy "${NOVOS_KERNEL_ELF}" "${NOVOS_ESP}/EFI/NOVOS/NOVOS_kernel.elf"
    DEPENDS ${NOVOS_BOOT_EFI} ${NOVOS_KERNEL_ELF}
)

add_custom_target(novos_uefi_image ALL
    DEPENDS ${NOVOS_ESP}/EFI/BOOT/BOOTX64.EFI
)
