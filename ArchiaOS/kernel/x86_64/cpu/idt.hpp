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
    unsigned long long user_rsp;
    unsigned long long user_ss;
};

extern "C" void idt_initialize();
extern "C" void idt_test_invalid_opcode();
extern "C" void idt_test_double_fault();
extern "C" ExceptionFrame* exception_dispatch(ExceptionFrame* frame);
extern "C" ExceptionFrame* irq_dispatch(ExceptionFrame* frame);
extern "C" void exception_expect_page_fault(unsigned long long recovery_rip, unsigned long long expected_cr2, unsigned long long error_mask, unsigned long long error_value);
extern "C" bool exception_page_fault_test_active();
extern "C" void exception_expect_invalid_opcode(unsigned long long rip);
extern "C" bool exception_invalid_opcode_test_active();

extern "C" void idt_load_current();
