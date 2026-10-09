#include "device.hpp"

static constexpr unsigned int DEVICE_MAX = 128;
static Device devices[DEVICE_MAX] = {};
static unsigned int count = 0;
static uint32_t next_id = 1;

extern "C" bool device_manager_initialize()
{
    for (auto& d : devices) d = {};
    count = 0;
    next_id = 1;
    return true;
}

extern "C" bool device_manager_register(
    DeviceType type, uint64_t resource0, uint64_t resource1, uint32_t* id)
{
    if (!id || count >= DEVICE_MAX)
        return false;

    for (unsigned int i = 0; i < DEVICE_MAX; ++i)
    {
        if (devices[i].id)
            continue;

        uint32_t id_value = next_id ? next_id : 1U;
        bool found_free_id = false;
        for (unsigned int attempt = 0; attempt <= DEVICE_MAX; ++attempt)
        {
            bool in_use = false;
            for (const auto& existing : devices)
            {
                if (existing.id == id_value)
                {
                    in_use = true;
                    break;
                }
            }
            if (!in_use)
            {
                found_free_id = true;
                break;
            }

            ++id_value;
            if (!id_value)
                id_value = 1U;
        }
        if (!found_free_id)
            return false;

        next_id = id_value + 1U;
        if (!next_id)
            next_id = 1U;
        devices[i] = {id_value, static_cast<uint32_t>(type), 1, resource0, resource1};
        *id = devices[i].id;
        ++count;
        return true;
    }
    return false;
}

extern "C" const Device* device_manager_get(uint32_t id)
{
    for (const auto& d : devices)
        if (d.id == id)
            return &d;
    return nullptr;
}

extern "C" unsigned int device_manager_count()
{
    return count;
}

extern "C" bool device_manager_test()
{
    if (!device_manager_initialize())
        return false;

    uint32_t id = 0;
    if (!device_manager_register(DEVICE_PCI, 0x1234, 0x5678, &id))
        return false;

    const Device* d = device_manager_get(id);
    next_id = UINT32_MAX;
    uint32_t wrapped_id = 0;
    if (!device_manager_register(DEVICE_PCI, 0, 0, &wrapped_id) ||
        wrapped_id != UINT32_MAX ||
        !device_manager_get(wrapped_id))
        return false;

    uint32_t post_wrap_id = 0;
    if (!device_manager_register(DEVICE_PCI, 1, 1, &post_wrap_id) ||
        post_wrap_id == 0 || post_wrap_id == id ||
        post_wrap_id == wrapped_id ||
        !device_manager_get(post_wrap_id))
        return false;

    return d && d->type == DEVICE_PCI &&
           d->resource0 == 0x1234 &&
           device_manager_count() == 3;
}
