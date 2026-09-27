#include "framebuffer.h"

static constexpr EFI_GUID gGraphicsOutputProtocolGuid = {
    0x9042a9de, 0x23dc, 0x4a38,
    {0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a}
};

EFI_STATUS discover_framebuffer(
    EFI_SYSTEM_TABLE* st,
    BootInfo* info)
{
    if (!st || !st->BootServices || !info)
        return EFI_INVALID_PARAMETER;

    auto locateProtocol =
        reinterpret_cast<EFI_LOCATE_PROTOCOL>(
            st->BootServices->LocateProtocol);

    EFI_GRAPHICS_OUTPUT_PROTOCOL* gop = nullptr;
    EFI_STATUS status = locateProtocol(
        const_cast<EFI_GUID*>(&gGraphicsOutputProtocolGuid),
        nullptr,
        reinterpret_cast<void**>(&gop));

    if (status != EFI_SUCCESS || !gop || !gop->Mode || !gop->Mode->Info)
        return status != EFI_SUCCESS ? status : EFI_UNSUPPORTED;

    if (gop->Mode->Info->PixelFormat !=
            PixelRedGreenBlueReserved8BitPerColor &&
        gop->Mode->Info->PixelFormat !=
            PixelBlueGreenRedReserved8BitPerColor)
        return EFI_UNSUPPORTED;

    const UINT32 width = gop->Mode->Info->HorizontalResolution;
    const UINT32 height = gop->Mode->Info->VerticalResolution;
    const UINT32 pixelsPerScanLine = gop->Mode->Info->PixelsPerScanLine;

    if (gop->Mode->FrameBufferBase == 0 ||
        width == 0 || height == 0 || pixelsPerScanLine < width ||
        pixelsPerScanLine > 0x3FFFFFFFU)
        return EFI_INVALID_PARAMETER;

    const UINT64 pitch = static_cast<UINT64>(pixelsPerScanLine) * 4ULL;
    if (height != 0 && pitch > (~0ULL / static_cast<UINT64>(height)))
        return EFI_INVALID_PARAMETER;

    const UINT64 requiredSize = pitch * static_cast<UINT64>(height);

    if (requiredSize > gop->Mode->FrameBufferSize)
        return EFI_INVALID_PARAMETER;

    info->framebuffer_base = gop->Mode->FrameBufferBase;
    info->framebuffer_size = gop->Mode->FrameBufferSize;
    info->framebuffer_width = width;
    info->framebuffer_height = height;
    info->framebuffer_pitch = static_cast<UINT32>(pitch);
    info->framebuffer_bpp = 32;
    info->framebuffer_pixel_format =
        static_cast<UINT32>(gop->Mode->Info->PixelFormat);

    return EFI_SUCCESS;
}
