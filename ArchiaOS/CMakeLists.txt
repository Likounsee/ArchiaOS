cmake_minimum_required(VERSION 3.20)
project(NovOS NONE)

set(NOVOS_ROOT ${CMAKE_CURRENT_SOURCE_DIR})
set(NOVOS_BUILD ${CMAKE_BINARY_DIR})

set(NOVOS_CLANG "" CACHE FILEPATH "Clang++ compiler")
set(NOVOS_LLD "" CACHE FILEPATH "LLD ELF linker")
set(NOVOS_LLD_LINK "" CACHE FILEPATH "LLD PE/COFF linker")

if(NOT NOVOS_CLANG)
    find_program(NOVOS_CLANG NAMES clang++.exe clang++)
endif()
if(NOT NOVOS_LLD)
    find_program(NOVOS_LLD NAMES ld.lld.exe ld.lld)
endif()
if(NOT NOVOS_LLD_LINK)
    find_program(NOVOS_LLD_LINK NAMES lld-link.exe lld-link)
endif()

if(NOT NOVOS_CLANG AND EXISTS "C:/Program Files/LLVM/bin/clang++.exe")
    set(NOVOS_CLANG "C:/Program Files/LLVM/bin/clang++.exe")
endif()
if(NOT NOVOS_LLD AND EXISTS "C:/Program Files/LLVM/bin/ld.lld.exe")
    set(NOVOS_LLD "C:/Program Files/LLVM/bin/ld.lld.exe")
endif()
if(NOT NOVOS_LLD_LINK AND EXISTS "C:/Program Files/LLVM/bin/lld-link.exe")
    set(NOVOS_LLD_LINK "C:/Program Files/LLVM/bin/lld-link.exe")
endif()

foreach(tool IN ITEMS NOVOS_CLANG NOVOS_LLD NOVOS_LLD_LINK)
    if(NOT ${tool})
        message(FATAL_ERROR "${tool} not found. Install LLVM or pass -D${tool}=...")
    endif()
endforeach()

set(NOVOS_BOOT_DIR ${NOVOS_ROOT}/boot/uefi/x86_64)
set(NOVOS_BOOT_INC ${NOVOS_ROOT}/boot/uefi/include)
set(NOVOS_COMMON ${NOVOS_ROOT}/common)
set(NOVOS_KERNEL_DIR ${NOVOS_ROOT}/kernel/x86_64/core)

set(NOVOS_BOOT_HEADERS
    ${NOVOS_BOOT_INC}/uefi.h
    ${NOVOS_BOOT_INC}/boot_debug.h
    ${NOVOS_BOOT_INC}/filesystem.h
    ${NOVOS_BOOT_INC}/elf_loader.h
    ${NOVOS_BOOT_INC}/framebuffer.h
    ${NOVOS_BOOT_INC}/memory_map.h
    ${NOVOS_BOOT_INC}/handoff.h
    ${NOVOS_COMMON}/boot_info.h
)

set(NOVOS_KERNEL_OBJ ${NOVOS_BUILD}/novos_kernel.obj)
set(NOVOS_KERNEL_ENTRY_OBJ ${NOVOS_BUILD}/novos_kernel_entry.obj)
set(NOVOS_KERNEL_ELF ${NOVOS_BUILD}/NOVOS_kernel.elf)

set(NOVOS_BOOT_MAIN_OBJ ${NOVOS_BUILD}/novos_boot_main.obj)
set(NOVOS_BOOT_DEBUG_OBJ ${NOVOS_BUILD}/novos_boot_debug.obj)
set(NOVOS_BOOT_FS_OBJ ${NOVOS_BUILD}/novos_boot_filesystem.obj)
set(NOVOS_BOOT_ELF_OBJ ${NOVOS_BUILD}/novos_boot_elf_loader.obj)
set(NOVOS_BOOT_FB_OBJ ${NOVOS_BUILD}/novos_boot_framebuffer.obj)
set(NOVOS_BOOT_MM_OBJ ${NOVOS_BUILD}/novos_boot_memory_map.obj)
set(NOVOS_BOOT_HANDOFF_OBJ ${NOVOS_BUILD}/novos_boot_handoff.obj)
set(NOVOS_BOOT_EFI ${NOVOS_BUILD}/BOOTX64.EFI)

set(NOVOS_ESP_DIR ${NOVOS_BUILD}/esp)
set(NOVOS_ESP_BOOT ${NOVOS_ESP_DIR}/EFI/BOOT/BOOTX64.EFI)
set(NOVOS_ESP_KERNEL ${NOVOS_ESP_DIR}/EFI/NOVOS/NOVOS_kernel.elf)

add_custom_command(
    OUTPUT ${NOVOS_KERNEL_OBJ}
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-none-elf
        -ffreestanding -fno-exceptions -fno-rtti -fno-stack-protector
        -mno-red-zone -mno-sse -mno-sse2
        -I"${NOVOS_COMMON}"
        -c "${NOVOS_KERNEL_DIR}/kernel.cpp" -o "${NOVOS_KERNEL_OBJ}"
    DEPENDS ${NOVOS_KERNEL_DIR}/kernel.cpp ${NOVOS_COMMON}/boot_info.h
)

add_custom_command(
    OUTPUT ${NOVOS_KERNEL_ENTRY_OBJ}
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-none-elf
        -ffreestanding -mno-red-zone
        -c "${NOVOS_KERNEL_DIR}/entry.S" -o "${NOVOS_KERNEL_ENTRY_OBJ}"
    DEPENDS ${NOVOS_KERNEL_DIR}/entry.S
)

add_custom_command(
    OUTPUT ${NOVOS_KERNEL_ELF}
    COMMAND "${NOVOS_LLD}" -m elf_x86_64
        -T "${NOVOS_KERNEL_DIR}/kernel.ld"
        "${NOVOS_KERNEL_OBJ}" "${NOVOS_KERNEL_ENTRY_OBJ}"
        -o "${NOVOS_KERNEL_ELF}"
    DEPENDS ${NOVOS_KERNEL_OBJ} ${NOVOS_KERNEL_ENTRY_OBJ}
            ${NOVOS_KERNEL_DIR}/kernel.ld
)

function(novos_boot_compile output source)
    add_custom_command(
        OUTPUT ${output}
        COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff
        -ffreestanding -fshort-wchar -fno-builtin
            -fno-exceptions -fno-rtti -fno-stack-protector
            -mno-red-zone -mno-sse -mno-sse2
            -DNOVOS_BOOT_DEBUG=1
            -I"${NOVOS_BOOT_INC}" -I"${NOVOS_COMMON}"
            -c "${source}" -o "${output}"
        DEPENDS ${source} ${NOVOS_BOOT_HEADERS}
    )
endfunction()

novos_boot_compile(${NOVOS_BOOT_MAIN_OBJ} ${NOVOS_BOOT_DIR}/main.cpp)
novos_boot_compile(${NOVOS_BOOT_DEBUG_OBJ} ${NOVOS_BOOT_DIR}/debug.cpp)
novos_boot_compile(${NOVOS_BOOT_FS_OBJ} ${NOVOS_BOOT_DIR}/filesystem.cpp)
novos_boot_compile(${NOVOS_BOOT_ELF_OBJ} ${NOVOS_BOOT_DIR}/elf_loader.cpp)
novos_boot_compile(${NOVOS_BOOT_FB_OBJ} ${NOVOS_BOOT_DIR}/framebuffer.cpp)
novos_boot_compile(${NOVOS_BOOT_MM_OBJ} ${NOVOS_BOOT_DIR}/memory_map.cpp)

add_custom_command(
    OUTPUT ${NOVOS_BOOT_HANDOFF_OBJ}
    COMMAND "${NOVOS_CLANG}" -target x86_64-pc-win32-coff
        -ffreestanding -c "${NOVOS_BOOT_DIR}/handoff.S"
        -o "${NOVOS_BOOT_HANDOFF_OBJ}"
    DEPENDS ${NOVOS_BOOT_DIR}/handoff.S ${NOVOS_BOOT_HEADERS}
)

add_custom_command(
    OUTPUT ${NOVOS_BOOT_EFI}
    COMMAND "${NOVOS_LLD_LINK}"
        /subsystem:efi_application
        /entry:efi_main
        /nodefaultlib
        /machine:x64
        "${NOVOS_BOOT_MAIN_OBJ}"
        "${NOVOS_BOOT_DEBUG_OBJ}"
        "${NOVOS_BOOT_FS_OBJ}"
        "${NOVOS_BOOT_ELF_OBJ}"
        "${NOVOS_BOOT_FB_OBJ}"
        "${NOVOS_BOOT_MM_OBJ}"
        "${NOVOS_BOOT_HANDOFF_OBJ}"
        /out:"${NOVOS_BOOT_EFI}"
    DEPENDS
        ${NOVOS_BOOT_MAIN_OBJ}
        ${NOVOS_BOOT_DEBUG_OBJ}
        ${NOVOS_BOOT_FS_OBJ}
        ${NOVOS_BOOT_ELF_OBJ}
        ${NOVOS_BOOT_FB_OBJ}
        ${NOVOS_BOOT_MM_OBJ}
        ${NOVOS_BOOT_HANDOFF_OBJ}
        ${NOVOS_KERNEL_ELF}
)

add_custom_command(
    OUTPUT ${NOVOS_ESP_BOOT}
    COMMAND ${CMAKE_COMMAND} -E make_directory "${NOVOS_ESP_DIR}/EFI/BOOT"
    COMMAND ${CMAKE_COMMAND} -E make_directory "${NOVOS_ESP_DIR}/EFI/NOVOS"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${NOVOS_BOOT_EFI}" "${NOVOS_ESP_BOOT}"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${NOVOS_KERNEL_ELF}" "${NOVOS_ESP_KERNEL}"
    DEPENDS ${NOVOS_BOOT_EFI} ${NOVOS_KERNEL_ELF}
)

add_custom_target(novos_uefi_image ALL
    DEPENDS ${NOVOS_ESP_BOOT}
)

add_custom_target(novos_uefi_artifacts
    DEPENDS ${NOVOS_BOOT_EFI} ${NOVOS_KERNEL_ELF} ${NOVOS_ESP_BOOT}
)
