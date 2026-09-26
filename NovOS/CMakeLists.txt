cmake_minimum_required(VERSION 4.0)
project(NovOS LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_executable(novos_kernel kernel/x86_64/kernel.cpp)

set_target_properties(novos_kernel PROPERTIES
    OUTPUT_NAME "novos_kernel"
    SUFFIX ".elf"
)

