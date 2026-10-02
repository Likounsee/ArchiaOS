#include "graphics.hpp"

extern "C" bool graphics_initialize(GraphicsSurface* surface)
{
    return surface && surface->pixels && surface->width &&
           surface->height && surface->pitch_pixels >= surface->width;
}

extern "C" void graphics_clear(GraphicsSurface* surface, uint32_t pixel)
{
    if (!graphics_initialize(surface))
        return;
    for (uint32_t y = 0; y < surface->height; ++y)
        for (uint32_t x = 0; x < surface->width; ++x)
            surface->pixels[static_cast<uint64_t>(y) * surface->pitch_pixels + x] = pixel;
}

extern "C" void graphics_fill_rect(
    GraphicsSurface* surface, uint32_t x, uint32_t y,
    uint32_t w, uint32_t h, uint32_t pixel)
{
    if (!graphics_initialize(surface) || !w || !h)
        return;

    uint64_t right = static_cast<uint64_t>(x) + w;
    uint64_t bottom = static_cast<uint64_t>(y) + h;
    if (x >= surface->width || y >= surface->height)
        return;

    if (right > surface->width) right = surface->width;
    if (bottom > surface->height) bottom = surface->height;

    for (uint32_t py = y; py < bottom; ++py)
        for (uint32_t px = x; px < right; ++px)
            surface->pixels[static_cast<uint64_t>(py) * surface->pitch_pixels + px] = pixel;
}

extern "C" void graphics_frame(GraphicsSurface* surface)
{
    if (!graphics_initialize(surface))
        return;

    graphics_clear(surface, 0x00101820U);
    graphics_fill_rect(surface, 0, 0, surface->width, 40, 0x00242A34U);

    const uint32_t panel_w = surface->width > 720 ? 640 : surface->width - 32;
    const uint32_t panel_h = surface->height > 520 ? 420 : surface->height - 72;
    graphics_fill_rect(surface, 16, 56, panel_w, panel_h, 0x00F2F4F7U);
    graphics_fill_rect(surface, 16, 56, panel_w, 32, 0x003A4656U);
}

extern "C" bool graphics_test()
{
    static uint32_t buffer[64 * 48] = {};
    GraphicsSurface s{buffer, 64, 48, 64};
    if (!graphics_initialize(&s))
        return false;
    graphics_clear(&s, 0x11U);
    graphics_fill_rect(&s, 4, 5, 8, 9, 0x22U);
    if (buffer[0] != 0x11U || buffer[5 * 64 + 4] != 0x22U)
        return false;
    graphics_fill_rect(&s, 60, 40, 16, 16, 0x33U);
    return buffer[47 * 64 + 63] == 0x33U;
}
