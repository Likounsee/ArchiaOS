#pragma once

#include <stdint.h>

struct InputEvent
{
    uint16_t type;
    uint16_t code;
    uint32_t value;
};

static constexpr uint16_t INPUT_EVENT_KEYBOARD = 1;
static constexpr uint16_t INPUT_EVENT_MOUSE = 2;

extern "C" bool input_initialize();
extern "C" bool input_push(const InputEvent* event);
extern "C" bool input_pop(InputEvent* event);
extern "C" unsigned int input_pending();
extern "C" bool input_test();
