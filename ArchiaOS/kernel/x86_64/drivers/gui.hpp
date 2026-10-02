#pragma once

#include <stdint.h>
#include "../drivers/graphics.hpp"
#include "../drivers/input.hpp"

struct GuiWindow
{
    bool used;
    uint32_t id;
    uint32_t x;
    uint32_t y;
    uint32_t width;
    uint32_t height;
    uint32_t title_pixel;
    uint32_t body_pixel;
};

extern "C" bool gui_initialize();
extern "C" int gui_create_window(uint32_t x, uint32_t y, uint32_t width, uint32_t height);
extern "C" bool gui_destroy_window(uint32_t id);
extern "C" bool gui_dispatch_input(const InputEvent* event);
extern "C" void gui_render(GraphicsSurface* surface);
extern "C" unsigned int gui_window_count();
extern "C" bool gui_test();
