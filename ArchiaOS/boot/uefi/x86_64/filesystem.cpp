#include "filesystem.h"
#include "boot_debug.h"

static constexpr EFI_GUID gLoadedImageProtocolGuid = {
    0x5b1b31a1, 0x9562, 0x11d2,
    {0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b}
};

static constexpr EFI_GUID gSimpleFileSystemProtocolGuid = {
    0x964e5b22, 0x6459, 0x11d2,
    {0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b}
};

EFI_STATUS open_kernel_file(
    EFI_HANDLE imageHandle,
    EFI_SYSTEM_TABLE* systemTable,
    EFI_FILE_PROTOCOL** outFile)
{
    if (!systemTable || !systemTable->BootServices || !outFile)
        return EFI_INVALID_PARAMETER;

    *outFile = nullptr;
    EFI_BOOT_SERVICES* bs = systemTable->BootServices;

    auto handleProtocol =
        reinterpret_cast<EFI_HANDLE_PROTOCOL>(bs->HandleProtocol);

    EFI_LOADED_IMAGE_PROTOCOL* loadedImage = nullptr;
    EFI_STATUS status = handleProtocol(
        imageHandle,
        const_cast<EFI_GUID*>(&gLoadedImageProtocolGuid),
        reinterpret_cast<void**>(&loadedImage));

    if (status != EFI_SUCCESS || !loadedImage)
        return status;

    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL* fs = nullptr;
    status = handleProtocol(
        loadedImage->DeviceHandle,
        const_cast<EFI_GUID*>(&gSimpleFileSystemProtocolGuid),
        reinterpret_cast<void**>(&fs));

    if (status != EFI_SUCCESS || !fs)
        return status;

    EFI_FILE_PROTOCOL* root = nullptr;
    status = fs->OpenVolume(fs, &root);
    if (status != EFI_SUCCESS || !root)
        return status;

    static CHAR16 path[] = {
        0x005C, 0x0045, 0x0046, 0x0049, 0x005C,
        0x004F, 0x0053,
        0x005C,
        0x004F, 0x0053,
        0x005F, 0x006B, 0x0065, 0x0072, 0x006E, 0x0065, 0x006C,
        0x002E, 0x0065, 0x006C, 0x0066, 0x0000
    };

    status = root->Open(
        root,
        outFile,
        path,
        EFI_FILE_MODE_READ,
        0);

    root->Close(root);

    if (status == EFI_SUCCESS)
        boot_debug("filesystem: kernel opened\r\n");

    return status;
}
