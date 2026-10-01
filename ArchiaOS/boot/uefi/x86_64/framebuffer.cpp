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
    {
        /* A text-only firmware is valid; graphics is optional to the kernel. */
        info->framebuffer_base = 0;
        info->framebuffer_size = 0;
        return EFI_SUCCESS;
    }

    auto* modeInfo = gop->Mode->Info;
    if (modeInfo->PixelFormat == PixelBltOnly)
    {
        info->framebuffer_base = 0;
        info->framebuffer_size = 0;
        return EFI_SUCCESS;
    }

    if (modeInfo->PixelFormat == PixelBitMask)
    {
        const EFI_PIXEL_BITMASK& m = modeInfo->PixelInformation;
        const bool rgb = m.RedMask == 0x000000FFU &&
                         m.GreenMask == 0x0000FF00U &&
                         m.BlueMask == 0x00FF0000U;
        const bool bgr = m.BlueMask == 0x000000FFU &&
                         m.GreenMask == 0x0000FF00U &&
                         m.RedMask == 0x00FF0000U;
        if (!rgb && !bgr)
            return EFI_UNSUPPORTED;
    }
    else if (modeInfo->PixelFormat != PixelRedGreenBlueReserved8BitPerColor &&
             modeInfo->PixelFormat != PixelBlueGreenRedReserved8BitPerColor)
    {
        return EFI_UNSUPPORTED;
    }

    const UINT32 width = modeInfo->HorizontalResolution;
    const UINT32 height = modeInfo->VerticalResolution;
    const UINT32 pixelsPerScanLine = modeInfo->PixelsPerScanLine;

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
    if (modeInfo->PixelFormat == PixelBitMask)
    {
        const EFI_PIXEL_BITMASK& m = modeInfo->PixelInformation;
        info->framebuffer_pixel_format =
            (m.RedMask == 0x000000FFU) ? 0U : 1U;
    }
    else
    {
        info->framebuffer_pixel_format =
            static_cast<UINT32>(modeInfo->PixelFormat);
    }

    return EFI_SUCCESS;
}
