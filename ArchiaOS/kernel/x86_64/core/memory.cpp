extern "C" void* memset(void* destination, int value, unsigned long long size)
{
    unsigned char* bytes = reinterpret_cast<unsigned char*>(destination);
    unsigned char byte = static_cast<unsigned char>(value);

    for (unsigned long long i = 0; i < size; ++i)
        bytes[i] = byte;

    return destination;
}

extern "C" void* memcpy(void* destination, const void* source, unsigned long long size)
{
    unsigned char* dst = reinterpret_cast<unsigned char*>(destination);
    const unsigned char* src =
        reinterpret_cast<const unsigned char*>(source);

    for (unsigned long long i = 0; i < size; ++i)
        dst[i] = src[i];

    return destination;
}

extern "C" void* memmove(void* destination, const void* source, unsigned long long size)
{
    unsigned char* dst = reinterpret_cast<unsigned char*>(destination);
    const unsigned char* src =
        reinterpret_cast<const unsigned char*>(source);

    if (dst == src || size == 0)
        return destination;

    if (dst < src)
    {
        for (unsigned long long i = 0; i < size; ++i)
            dst[i] = src[i];
    }
    else
    {
        for (unsigned long long i = size; i > 0; --i)
            dst[i - 1] = src[i - 1];
    }

    return destination;
}
