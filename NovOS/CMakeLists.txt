cmake_minimum_required(VERSION 4.0)
project(NovOS LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_executable(novos_kernel
    kernel/x86_64/core/kernel.cpp
    kernel/x86_64/cpu/exception.cpp
    kernel/x86_64/cpu/irq.cpp
    kernel/x86_64/cpu/lapic.cpp
    kernel/x86_64/cpu/gdt.S
    kernel/x86_64/cpu/idt.S
    kernel/x86_64/cpu/tss.S
    kernel/x86_64/memory/pmm.cpp
    kernel/x86_64/memory/pmm_test.cpp
    kernel/x86_64/memory/paging.cpp
    kernel/x86_64/memory/paging_test.cpp
    kernel/x86_64/core/entry.S
)

target_include_directories(novos_kernel PRIVATE
    common
    kernel/x86_64/cpu
    kernel/x86_64/memory
)

set_target_properties(novos_kernel PROPERTIES
    OUTPUT_NAME "novos_kernel"
    SUFFIX ".elf"
)
