#pragma once

#include <stdint.h>

struct GraphicsSurface
{
    volatile uint32_t* pixels;
    uint32_t width;
    uint32_t height;
    uint32_t pitch_pixels;
};

extern "C" bool graphics_initialize(GraphicsSurface* surface);
extern "C" void graphics_clear(GraphicsSurface* surface, uint32_t pixel);
extern "C" void graphics_fill_rect(GraphicsSurface* surface, uint32_t x, uint32_t y, uint32_t w, uint32_t h, uint32_t pixel);
extern "C" void graphics_frame(GraphicsSurface* surface);
extern "C" bool graphics_test();
