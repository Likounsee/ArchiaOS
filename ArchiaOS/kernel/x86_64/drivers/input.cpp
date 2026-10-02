#include "input.hpp"

static constexpr unsigned int INPUT_QUEUE_SIZE = 256;
static InputEvent queue[INPUT_QUEUE_SIZE] = {};
static volatile unsigned int head = 0;
static volatile unsigned int tail = 0;
static bool initialized = false;

extern "C" bool input_initialize()
{
    head = 0;
    tail = 0;
    initialized = true;
    return true;
}

extern "C" bool input_push(const InputEvent* event)
{
    if (!initialized || !event)
        return false;

    const uint64_t flags = []() -> uint64_t {
        uint64_t f; asm volatile("pushfq; popq %0; cli" : "=r"(f) : : "memory"); return f;
    }();

    const unsigned int next = (head + 1) % INPUT_QUEUE_SIZE;
    if (next == tail)
    {
        if (flags & (1ULL << 9)) asm volatile("sti");
        return false;
    }
    queue[head] = *event;
    head = next;
    if (flags & (1ULL << 9)) asm volatile("sti");
    return true;
}

extern "C" bool input_pop(InputEvent* event)
{
    if (!initialized || !event || tail == head)
        return false;

    const uint64_t flags = []() -> uint64_t {
        uint64_t f; asm volatile("pushfq; popq %0; cli" : "=r"(f) : : "memory"); return f;
    }();
    if (tail == head)
    {
        if (flags & (1ULL << 9)) asm volatile("sti");
        return false;
    }
    *event = queue[tail];
    tail = (tail + 1) % INPUT_QUEUE_SIZE;
    if (flags & (1ULL << 9)) asm volatile("sti");
    return true;
}

extern "C" unsigned int input_pending()
{
    return (head + INPUT_QUEUE_SIZE - tail) % INPUT_QUEUE_SIZE;
}

extern "C" bool input_test()
{
    if (!input_initialize())
        return false;
    const InputEvent event{INPUT_EVENT_KEYBOARD, 0x1E, 1};
    if (!input_push(&event) || input_pending() != 1)
        return false;
    InputEvent received{};
    return input_pop(&received) &&
           received.type == event.type &&
           received.code == event.code &&
           received.value == event.value &&
           input_pending() == 0;
}
