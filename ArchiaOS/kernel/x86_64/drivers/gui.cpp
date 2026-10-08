#include "gui.hpp"

static constexpr unsigned int GUI_MAX_WINDOWS = 16;
static GuiWindow windows[GUI_MAX_WINDOWS] = {};
static uint32_t next_id = 1;
static unsigned int count = 0;
static bool initialized = false;

extern "C" bool gui_initialize()
{
    if (initialized)
        return true;

    for (auto& window : windows) window = {};
    next_id = 1;
    count = 0;
    initialized = true;
    return true;
}

extern "C" int gui_create_window(uint32_t x, uint32_t y, uint32_t width, uint32_t height)
{
    if (!initialized || !width || !height)
        return -1;

    for (unsigned int i = 0; i < GUI_MAX_WINDOWS; ++i)
    {
        if (!windows[i].used)
        {
            const uint32_t id_value = next_id ? next_id : 1U;
            next_id = id_value + 1U;
            if (!next_id)
                next_id = 1U;
            windows[i] = {
                true, id_value, x, y, width, height,
                0x003A4656U, 0x00F2F4F7U
            };
            ++count;
            return static_cast<int>(windows[i].id);
        }
    }
    return -1;
}

extern "C" bool gui_destroy_window(uint32_t id)
{
    for (auto& window : windows)
    {
        if (window.used && window.id == id)
        {
            window = {};
            --count;
            return true;
        }
    }
    return false;
}

extern "C" bool gui_dispatch_input(const InputEvent* event)
{
    if (!initialized || !event)
        return false;
    return event->type == INPUT_EVENT_KEYBOARD;
}

extern "C" void gui_render(GraphicsSurface* surface)
{
    if (!graphics_initialize(surface))
        return;

    graphics_frame(surface);
    for (unsigned int i = 0; i < GUI_MAX_WINDOWS; ++i)
    {
        if (!windows[i].used)
            continue;
        graphics_fill_rect(surface, windows[i].x, windows[i].y,
                           windows[i].width, 28, windows[i].title_pixel);
        graphics_fill_rect(surface, windows[i].x, windows[i].y + 28,
                           windows[i].width, windows[i].height - 28,
                           windows[i].body_pixel);
    }
}

extern "C" unsigned int gui_window_count()
{
    return count;
}

extern "C" bool gui_test()
{
    if (!gui_initialize())
        return false;

    const int first = gui_create_window(10, 10, 100, 80);
    const int second = gui_create_window(30, 30, 120, 90);
    if (first < 1 || second < 1 || first == second || gui_window_count() != 2)
        return false;

    InputEvent event{INPUT_EVENT_KEYBOARD, 0x1E, 1};
    if (!gui_dispatch_input(&event) || !gui_destroy_window(static_cast<uint32_t>(first)))
        return false;

    return gui_window_count() == 1;
}
