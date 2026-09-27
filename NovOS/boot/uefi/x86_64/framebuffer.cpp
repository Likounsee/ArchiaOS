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

    info->framebuffer_base = gop->Mode->FrameBufferBase;
    info->framebuffer_size = gop->Mode->FrameBufferSize;
    info->framebuffer_width = gop->Mode->Info->HorizontalResolution;
    info->framebuffer_height = gop->Mode->Info->VerticalResolution;
    info->framebuffer_pitch =
        gop->Mode->Info->PixelsPerScanLine * 4;
    info->framebuffer_bpp = 32;
    info->framebuffer_pixel_format =
        static_cast<UINT32>(gop->Mode->Info->PixelFormat);

    return EFI_SUCCESS;
}
