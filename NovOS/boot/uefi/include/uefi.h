#pragma once

using UINT8   = unsigned char;
using UINT16  = unsigned short;
using UINT32  = unsigned int;
using UINT64  = unsigned long long;
using UINTN   = UINT64;
using INT64   = long long;
using BOOLEAN = UINT8;

using EFI_STATUS = UINT64;
using EFI_HANDLE = void*;
using EFI_EVENT  = void*;

#define EFI_SUCCESS 0

#define EFI_FILE_MODE_READ 0x0000000000000001ULL
#define EFI_ALLOCATE_ANY_PAGES     0
#define EFI_ALLOCATE_MAX_ADDRESS   1
#define EFI_ALLOCATE_ADDRESS       2

#define EFI_LOADER_DATA            4

#define EFI_ERROR_BIT              0x8000000000000000ULL
#define EFIERR(a)                  (EFI_ERROR_BIT | (a))
#define EFI_BUFFER_TOO_SMALL       EFIERR(5)
#define EFI_INVALID_PARAMETER      EFIERR(2)

#define EFI_MEMORY_DESCRIPTOR_VERSION 1
struct EFI_MEMORY_DESCRIPTOR
{
    UINT32 Type;
    UINT32 Reserved;
    UINT64 PhysicalStart;
    UINT64 VirtualStart;
    UINT64 NumberOfPages;
    UINT64 Attribute;
};

using EFI_GET_MEMORY_MAP =
    EFI_STATUS (*)(
        UINTN*,
        EFI_MEMORY_DESCRIPTOR*,
        UINTN*,
        UINTN*,
        UINT32*
    );

using EFI_ALLOCATE_POOL =
    EFI_STATUS (*)(
        UINT32,
        UINTN,
        void**
    );

using EFI_EXIT_BOOT_SERVICES =
    EFI_STATUS (*)(
        EFI_HANDLE,
        UINT64
    );


struct EFI_SYSTEM_TABLE;
struct EFI_BOOT_SERVICES;
struct EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL;
struct EFI_SIMPLE_FILE_SYSTEM_PROTOCOL;
struct EFI_FILE_PROTOCOL;
struct EFI_LOADED_IMAGE_PROTOCOL;

using EFI_TEXT_STRING =
    EFI_STATUS (*)(EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL*, UINT16*);

using EFI_HANDLE_PROTOCOL =
    EFI_STATUS (*)(EFI_HANDLE, void*, void**);

using EFI_LOCATE_PROTOCOL =
    EFI_STATUS (*)(void*, void*, void**);

using EFI_LOCATE_DEVICE_PATH =
    EFI_STATUS (*)(
        void*,
        void**,
        EFI_HANDLE*
    );

using EFI_FILE_OPEN =
    EFI_STATUS (*)(
        EFI_FILE_PROTOCOL*,
        EFI_FILE_PROTOCOL**,
        UINT16*,
        UINT64,
        UINT64
    );

using EFI_FILE_CLOSE =
    EFI_STATUS (*)(EFI_FILE_PROTOCOL*);

using EFI_FILE_READ =
    EFI_STATUS (*)(
        EFI_FILE_PROTOCOL*,
        UINTN*,
        void*
    );

using EFI_FILE_GET_INFO =
    EFI_STATUS (*)(
        EFI_FILE_PROTOCOL*,
        void*,
        UINTN*,
        void*
    );
using EFI_FILE_SET_POSITION =
    EFI_STATUS (*)(
        EFI_FILE_PROTOCOL*,
        UINT64
    );
using EFI_ALLOCATE_PAGES =
    EFI_STATUS (*)(
        UINT32,
        UINT32,
        UINTN,
        UINT64*
    );

using EFI_SIMPLE_FILE_SYSTEM_OPEN_VOLUME =
    EFI_STATUS (*)(
        EFI_SIMPLE_FILE_SYSTEM_PROTOCOL*,
        EFI_FILE_PROTOCOL**
    );

struct EFI_TABLE_HEADER
{
    UINT64 Signature;
    UINT32 Revision;
    UINT32 HeaderSize;
    UINT32 CRC32;
    UINT32 Reserved;
};

struct EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL
{
    void* Reset;
    EFI_TEXT_STRING OutputString;
};

struct EFI_BOOT_SERVICES
{
    EFI_TABLE_HEADER Hdr;

    void* RaiseTPL;
    void* RestoreTPL;
    void* AllocatePages;
    void* FreePages;
    void* GetMemoryMap;
    void* AllocatePool;
    void* FreePool;
    void* CreateEvent;
    void* SetTimer;
    void* WaitForEvent;
    void* SignalEvent;
    void* CloseEvent;
    void* CheckEvent;
    void* InstallProtocolInterface;
    void* ReinstallProtocolInterface;
    void* UninstallProtocolInterface;
    void* HandleProtocol;
    void* Reserved;
    void* RegisterProtocolNotify;
    void* LocateHandle;
    void* LocateDevicePath;
    void* InstallConfigurationTable;
    void* LoadImage;
    void* StartImage;
    void* Exit;
    void* UnloadImage;
    void* ExitBootServices;
    void* GetNextMonotonicCount;
    void* Stall;
    void* SetWatchdogTimer;
    void* ConnectController;
    void* DisconnectController;
    void* OpenProtocol;
    void* CloseProtocol;
    void* OpenProtocolInformation;
    void* ProtocolsPerHandle;
    void* LocateHandleBuffer;
    void* LocateProtocol;
    void* InstallMultipleProtocolInterfaces;
    void* UninstallMultipleProtocolInterfaces;
    void* CalculateCrc32;
    void* CopyMem;
    void* SetMem;
    void* CreateEventEx;
};

struct EFI_SIMPLE_FILE_SYSTEM_PROTOCOL
{
    UINT64 Revision;
    EFI_SIMPLE_FILE_SYSTEM_OPEN_VOLUME OpenVolume;
};

struct EFI_FILE_PROTOCOL
{
    UINT64 Revision;

    EFI_FILE_OPEN Open;
    EFI_FILE_CLOSE Close;
    void* Delete;
    EFI_FILE_READ Read;
    void* Write;
    void* GetPosition;
    void* SetPosition;
    EFI_FILE_GET_INFO GetInfo;
    void* SetInfo;
    void* Flush;
    void* OpenEx;
    void* ReadEx;
    void* WriteEx;
    void* FlushEx;
};

struct EFI_LOADED_IMAGE_PROTOCOL
{
    UINT32 Revision;
    EFI_HANDLE ParentHandle;
    EFI_SYSTEM_TABLE* SystemTable;
    EFI_HANDLE DeviceHandle;
    void* FilePath;
    void* Reserved;
    UINT32 LoadOptionsSize;
    void* LoadOptions;
    void* ImageBase;
    UINT64 ImageSize;
    UINT32 CodeType;
    UINT32 DataType;
    void* Unload;
};

struct EFI_SYSTEM_TABLE
{
    EFI_TABLE_HEADER Hdr;

    UINT16* FirmwareVendor;
    UINT32 FirmwareRevision;

    EFI_HANDLE ConsoleInHandle;
    void* ConIn;

    EFI_HANDLE ConsoleOutHandle;
    EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL* ConOut;

    EFI_HANDLE StandardErrorHandle;
    void* StdErr;

    void* RuntimeServices;
    EFI_BOOT_SERVICES* BootServices;

    UINTN NumberOfTableEntries;
    void* ConfigurationTable;
};

extern "C" EFI_STATUS efi_main(
    EFI_HANDLE ImageHandle,
    EFI_SYSTEM_TABLE* SystemTable
);

