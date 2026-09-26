#pragma once

struct ExceptionFrame
{
    unsigned long long r15;
    unsigned long long r14;
    unsigned long long r13;
    unsigned long long r12;
    unsigned long long r11;
    unsigned long long r10;
    unsigned long long r9;
    unsigned long long r8;
    unsigned long long rsi;
    unsigned long long rdi;
    unsigned long long rbp;
    unsigned long long rdx;
    unsigned long long rcx;
    unsigned long long rbx;
    unsigned long long rax;
    unsigned long long vector;
    unsigned long long error_code;
    unsigned long long rip;
    unsigned long long cs;
    unsigned long long rflags;
};

extern "C" void idt_initialize();
extern "C" void idt_test_double_fault();
extern "C" void exception_dispatch(ExceptionFrame* frame);
