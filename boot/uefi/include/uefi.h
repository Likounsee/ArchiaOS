#pragma once

using UINT8=unsigned char; using UINT16=unsigned short; using UINT32=unsigned int;
using UINT64=unsigned long long; using UINTN=UINT64; using BOOLEAN=UINT8;
using EFI_STATUS=UINT64; using EFI_HANDLE=void*; using EFI_EVENT=void*;
using EFI_PHYSICAL_ADDRESS=UINT64; using EFI_VIRTUAL_ADDRESS=UINT64; using CHAR16=UINT16;

#if defined(__x86_64__) || defined(_M_X64)
#define EFIAPI __attribute__((ms_abi))
#else
#define EFIAPI
#endif

#define EFI_SUCCESS 0ULL
#define EFI_ERROR_BIT 0x8000000000000000ULL
#define EFIERR(a) (EFI_ERROR_BIT | static_cast<UINT64>(a))
#define EFI_INVALID_PARAMETER EFIERR(2)
#define EFI_UNSUPPORTED EFIERR(3)
#define EFI_BUFFER_TOO_SMALL EFIERR(5)
#define EFI_OUT_OF_RESOURCES EFIERR(9)
#define EFI_NOT_FOUND EFIERR(14)
#define EFI_FILE_MODE_READ 1ULL

enum EFI_ALLOCATE_TYPE:UINT32 { EFI_ALLOCATE_ANY_PAGES=0, EFI_ALLOCATE_MAX_ADDRESS=1, EFI_ALLOCATE_ADDRESS=2 };
enum EFI_MEMORY_TYPE:UINT32 {
 EfiReservedMemoryType=0,EfiLoaderCode=1,EfiLoaderData=2,EfiBootServicesCode=3,
 EfiBootServicesData=4,EfiRuntimeServicesCode=5,EfiRuntimeServicesData=6,
 EfiConventionalMemory=7,EfiUnusableMemory=8,EfiACPIReclaimMemory=9,
 EfiACPIMemoryNVS=10,EfiMemoryMappedIO=11,EfiMemoryMappedIOPortSpace=12,
 EfiPalCode=13,EfiPersistentMemory=14,EfiMaxMemoryType=15
};

struct EFI_GUID { UINT32 Data1; UINT16 Data2; UINT16 Data3; UINT8 Data4[8]; };
struct EFI_TABLE_HEADER { UINT64 Signature; UINT32 Revision; UINT32 HeaderSize; UINT32 CRC32; UINT32 Reserved; };
struct EFI_CONFIGURATION_TABLE { EFI_GUID VendorGuid; void* VendorTable; };
struct EFI_MEMORY_DESCRIPTOR { UINT32 Type; UINT32 Pad; UINT64 PhysicalStart; UINT64 VirtualStart; UINT64 NumberOfPages; UINT64 Attribute; };

struct EFI_SYSTEM_TABLE; struct EFI_BOOT_SERVICES; struct EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL;
struct EFI_SIMPLE_FILE_SYSTEM_PROTOCOL; struct EFI_FILE_PROTOCOL; struct EFI_LOADED_IMAGE_PROTOCOL;
struct EFI_GRAPHICS_OUTPUT_PROTOCOL; struct EFI_GRAPHICS_OUTPUT_MODE_INFORMATION;

using EFI_TEXT_STRING=EFI_STATUS(EFIAPI*)(EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL*,CHAR16*);
using EFI_HANDLE_PROTOCOL=EFI_STATUS(EFIAPI*)(EFI_HANDLE,EFI_GUID*,void**);
using EFI_LOCATE_PROTOCOL=EFI_STATUS(EFIAPI*)(EFI_GUID*,void*,void**);
using EFI_ALLOCATE_PAGES=EFI_STATUS(EFIAPI*)(EFI_ALLOCATE_TYPE,EFI_MEMORY_TYPE,UINTN,EFI_PHYSICAL_ADDRESS*);
using EFI_ALLOCATE_POOL=EFI_STATUS(EFIAPI*)(EFI_MEMORY_TYPE,UINTN,void**);
using EFI_FREE_POOL=EFI_STATUS(EFIAPI*)(void*);
using EFI_GET_MEMORY_MAP=EFI_STATUS(EFIAPI*)(UINTN*,EFI_MEMORY_DESCRIPTOR*,UINTN*,UINTN*,UINT32*);
using EFI_EXIT_BOOT_SERVICES=EFI_STATUS(EFIAPI*)(EFI_HANDLE,UINTN);
using EFI_FILE_OPEN=EFI_STATUS(EFIAPI*)(EFI_FILE_PROTOCOL*,EFI_FILE_PROTOCOL**,CHAR16*,UINT64,UINT64);
using EFI_FILE_CLOSE=EFI_STATUS(EFIAPI*)(EFI_FILE_PROTOCOL*);
using EFI_FILE_READ=EFI_STATUS(EFIAPI*)(EFI_FILE_PROTOCOL*,UINTN*,void*);
using EFI_FILE_GET_POSITION=EFI_STATUS(EFIAPI*)(EFI_FILE_PROTOCOL*,UINT64*);
using EFI_FILE_SET_POSITION=EFI_STATUS(EFIAPI*)(EFI_FILE_PROTOCOL*,UINT64);
using EFI_SIMPLE_FILE_SYSTEM_OPEN_VOLUME=EFI_STATUS(EFIAPI*)(EFI_SIMPLE_FILE_SYSTEM_PROTOCOL*,EFI_FILE_PROTOCOL**);
using EFI_GRAPHICS_OUTPUT_PROTOCOL_QUERY_MODE=EFI_STATUS(EFIAPI*)(EFI_GRAPHICS_OUTPUT_PROTOCOL*,UINT32,UINTN*,EFI_GRAPHICS_OUTPUT_MODE_INFORMATION**);
using EFI_GRAPHICS_OUTPUT_PROTOCOL_SET_MODE=EFI_STATUS(EFIAPI*)(EFI_GRAPHICS_OUTPUT_PROTOCOL*,UINT32);
using EFI_GRAPHICS_OUTPUT_PROTOCOL_BLT=EFI_STATUS(EFIAPI*)(EFI_GRAPHICS_OUTPUT_PROTOCOL*,void*,UINT32,UINTN,UINTN,UINTN,UINTN,UINTN,UINTN,UINTN);

struct EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL { void* Reset; EFI_TEXT_STRING OutputString; };
struct EFI_BOOT_SERVICES {
 EFI_TABLE_HEADER Hdr; void* RaiseTPL; void* RestoreTPL; EFI_ALLOCATE_PAGES AllocatePages; void* FreePages;
 EFI_GET_MEMORY_MAP GetMemoryMap; EFI_ALLOCATE_POOL AllocatePool; EFI_FREE_POOL FreePool;
 void* CreateEvent; void* SetTimer; void* WaitForEvent; void* SignalEvent; void* CloseEvent; void* CheckEvent;
 void* InstallProtocolInterface; void* ReinstallProtocolInterface; void* UninstallProtocolInterface;
 EFI_HANDLE_PROTOCOL HandleProtocol; void* Reserved; void* RegisterProtocolNotify; void* LocateHandle;
 void* LocateDevicePath; void* InstallConfigurationTable; void* LoadImage; void* StartImage; void* Exit;
 void* UnloadImage; EFI_EXIT_BOOT_SERVICES ExitBootServices; void* GetNextMonotonicCount; void* Stall;
 void* SetWatchdogTimer; void* ConnectController; void* DisconnectController; void* OpenProtocol; void* CloseProtocol;
 void* OpenProtocolInformation; void* ProtocolsPerHandle; void* LocateHandleBuffer; EFI_LOCATE_PROTOCOL LocateProtocol;
 void* InstallMultipleProtocolInterfaces; void* UninstallMultipleProtocolInterfaces; void* CalculateCrc32;
 void* CopyMem; void* SetMem; void* CreateEventEx;
};
struct EFI_SYSTEM_TABLE {
 EFI_TABLE_HEADER Hdr; CHAR16* FirmwareVendor; UINT32 FirmwareRevision; UINT32 _pad0;
 EFI_HANDLE ConsoleInHandle; void* ConIn; EFI_HANDLE ConsoleOutHandle; EFI_SIMPLE_TEXT_OUTPUT_PROTOCOL* ConOut;
 EFI_HANDLE StandardErrorHandle; void* StdErr; void* RuntimeServices; EFI_BOOT_SERVICES* BootServices;
 UINTN NumberOfTableEntries; EFI_CONFIGURATION_TABLE* ConfigurationTable;
};
struct EFI_LOADED_IMAGE_PROTOCOL {
 UINT32 Revision; EFI_HANDLE ParentHandle; EFI_SYSTEM_TABLE* SystemTable; EFI_HANDLE DeviceHandle; void* FilePath;
 void* Reserved; UINT32 LoadOptionsSize; void* LoadOptions; void* ImageBase; UINT64 ImageSize; UINT32 CodeType; UINT32 DataType; void* Unload;
};
struct EFI_SIMPLE_FILE_SYSTEM_PROTOCOL { UINT64 Revision; EFI_SIMPLE_FILE_SYSTEM_OPEN_VOLUME OpenVolume; };
struct EFI_FILE_PROTOCOL {
 UINT64 Revision; EFI_FILE_OPEN Open; EFI_FILE_CLOSE Close; void* Delete; EFI_FILE_READ Read; void* Write;
 EFI_FILE_GET_POSITION GetPosition; EFI_FILE_SET_POSITION SetPosition; void* GetInfo; void* SetInfo; void* Flush;
 void* OpenEx; void* ReadEx; void* WriteEx; void* FlushEx;
};
enum EFI_GRAPHICS_PIXEL_FORMAT:UINT32 { PixelRedGreenBlueReserved8BitPerColor=0,PixelBlueGreenRedReserved8BitPerColor=1,PixelBitMask=2,PixelBltOnly=3,PixelFormatMax=4 };
struct EFI_PIXEL_BITMASK { UINT32 RedMask; UINT32 GreenMask; UINT32 BlueMask; UINT32 ReservedMask; };
struct EFI_GRAPHICS_OUTPUT_MODE_INFORMATION {
 UINT32 Version; UINT32 HorizontalResolution; UINT32 VerticalResolution; EFI_GRAPHICS_PIXEL_FORMAT PixelFormat;
 EFI_PIXEL_BITMASK PixelInformation; UINT32 PixelsPerScanLine;
};
struct EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE {
 UINT32 MaxMode; UINT32 Mode; EFI_GRAPHICS_OUTPUT_MODE_INFORMATION* Info; UINTN SizeOfInfo;
 EFI_PHYSICAL_ADDRESS FrameBufferBase; UINTN FrameBufferSize;
};
struct EFI_GRAPHICS_OUTPUT_PROTOCOL { EFI_GRAPHICS_OUTPUT_PROTOCOL_QUERY_MODE QueryMode; EFI_GRAPHICS_OUTPUT_PROTOCOL_SET_MODE SetMode; EFI_GRAPHICS_OUTPUT_PROTOCOL_BLT Blt; EFI_GRAPHICS_OUTPUT_PROTOCOL_MODE* Mode; };

static_assert(sizeof(EFI_GUID)==16,"EFI_GUID ABI mismatch");
static_assert(sizeof(EFI_MEMORY_DESCRIPTOR)==40,"EFI_MEMORY_DESCRIPTOR ABI mismatch");
static_assert(sizeof(EFI_TABLE_HEADER)==24,"EFI_TABLE_HEADER ABI mismatch");
static_assert(sizeof(EFI_FILE_PROTOCOL)==120,"EFI_FILE_PROTOCOL ABI mismatch");

extern "C" EFI_STATUS EFIAPI efi_main(EFI_HANDLE,EFI_SYSTEM_TABLE*);
