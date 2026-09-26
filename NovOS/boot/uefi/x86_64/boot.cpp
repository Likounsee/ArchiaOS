#include "uefi.h"
#include "../../common/boot_info.h"

static void print(EFI_SYSTEM_TABLE* SystemTable, const char* text)
{
    UINT16 buffer[128];
    UINTN i = 0;

    while (text[i] != '\0' && i < 127)
    {
        buffer[i] = static_cast<UINT16>(text[i]);
        ++i;
    }

    buffer[i] = 0;

    SystemTable->ConOut->OutputString(
        SystemTable->ConOut,
        buffer
    );
}

static void print_hex64(EFI_SYSTEM_TABLE* SystemTable, UINT64 value)
{
    const char* digits = "0123456789ABCDEF";
    UINT16 buffer[19];

    buffer[0] = '0';
    buffer[1] = 'x';

    for (int i = 0; i < 16; ++i)
    {
        buffer[2 + i] =
            static_cast<UINT16>(
                digits[(value >> ((15 - i) * 4)) & 0xF]
            );
    }

    buffer[18] = 0;

    SystemTable->ConOut->OutputString(
        SystemTable->ConOut,
        buffer
    );
}

static void print_hex8(EFI_SYSTEM_TABLE* SystemTable, UINT8 value)
{
    const char* digits = "0123456789ABCDEF";
    UINT16 buffer[3];

    buffer[0] = static_cast<UINT16>(digits[(value >> 4) & 0xF]);
    buffer[1] = static_cast<UINT16>(digits[value & 0xF]);
    buffer[2] = 0;

    SystemTable->ConOut->OutputString(
        SystemTable->ConOut,
        buffer
    );
}

static void halt()
{
    for (;;)
        asm volatile ("hlt");
}

struct Elf64Header
{
    UINT8  e_ident[16];
    UINT16 e_type;
    UINT16 e_machine;
    UINT32 e_version;
    UINT64 e_entry;
    UINT64 e_phoff;
    UINT64 e_shoff;
    UINT32 e_flags;
    UINT16 e_ehsize;
    UINT16 e_phentsize;
    UINT16 e_phnum;
    UINT16 e_shentsize;
    UINT16 e_shnum;
    UINT16 e_shstrndx;
};

static constexpr UINT16 ELF_ET_EXEC = 2;
static constexpr UINT16 ELF_EM_X86_64 = 62;
static constexpr UINT8 ELF_CLASS_64 = 2;
static constexpr UINT8 ELF_DATA_LSB = 1;
struct Elf64ProgramHeader
{
    UINT32 p_type;
    UINT32 p_flags;
    UINT64 p_offset;
    UINT64 p_vaddr;
    UINT64 p_paddr;
    UINT64 p_filesz;
    UINT64 p_memsz;
    UINT64 p_align;
};

static constexpr UINT32 ELF_PT_LOAD = 1;

static inline void debug_char(char c)
{
    asm volatile (
        "outb %0, %1"
        :
        : "a"(c),
          "Nd"(static_cast<unsigned short>(0xE9))
    );
}

static void debug_str(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        debug_char(s[i]);
}

static void debug_hex64(UINT64 value)
{
    const char* digits = "0123456789ABCDEF";
    debug_char('0');
    debug_char('x');

    for (int i = 0; i < 16; ++i)
        debug_char(digits[(value >> ((15 - i) * 4)) & 0xF]);
}

/*
 * jump_to_kernel_with_bootinfo
 * 
 * Jumps to kernel entry point with BootInfo* in RDI (x86-64 ABI)
 */
[[noreturn]] static void jump_to_kernel_with_bootinfo(UINT64 entry, BootInfo* bootInfo)
{
    debug_str("JUMP STUB\n");

    asm volatile (
        "mov %0, %%rdi\n\t"  /* RDI = bootInfo (first argument) */
        "jmp *%1\n\t"        /* Jump to kernel entry */
        :
        : "r"(bootInfo),
          "r"(entry)
    );

    __builtin_unreachable();
}

extern "C" EFI_STATUS efi_main(
    EFI_HANDLE ImageHandle,
    EFI_SYSTEM_TABLE* SystemTable)
{
    print(SystemTable, "NOVOS BOOTLOADER STARTED\r\n");

    EFI_BOOT_SERVICES* bs = SystemTable->BootServices;

    auto AllocatePages =
        reinterpret_cast<EFI_ALLOCATE_PAGES>(bs->AllocatePages);

    if (AllocatePages == nullptr)
    {
        halt();
    }

    auto HandleProtocol =
        reinterpret_cast<EFI_HANDLE_PROTOCOL>(bs->HandleProtocol);

    static UINT8 LoadedImageGuid[16] = {
        0xA1, 0x31, 0x1B, 0x5B,
        0x62, 0x95,
        0xD2, 0x11,
        0x8E, 0x3F,
        0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B
    };

    static UINT8 SimpleFileSystemGuid[16] = {
        0x22, 0x5B, 0x4E, 0x96,
        0x59, 0x64,
        0xD2, 0x11,
        0x8E, 0x39,
        0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B
    };

    EFI_LOADED_IMAGE_PROTOCOL* loadedImage = nullptr;

    EFI_STATUS status = HandleProtocol(
        ImageHandle,
        LoadedImageGuid,
        reinterpret_cast<void**>(&loadedImage)
    );

    if (status != EFI_SUCCESS || loadedImage == nullptr)
    {
        halt();
    }

    EFI_SIMPLE_FILE_SYSTEM_PROTOCOL* fileSystem = nullptr;

    auto LocateProtocol =
        reinterpret_cast<EFI_LOCATE_PROTOCOL>(bs->LocateProtocol);

    status = LocateProtocol(
        SimpleFileSystemGuid,
        nullptr,
        reinterpret_cast<void**>(&fileSystem)
    );

    if (status != EFI_SUCCESS || fileSystem == nullptr)
    {
        halt();
    }

    EFI_FILE_PROTOCOL* root = nullptr;

    status = fileSystem->OpenVolume(
        fileSystem,
        &root
    );

    if (status != EFI_SUCCESS || root == nullptr)
    {
        halt();
    }

    static UINT16 kernelPath[] = {
        '\\','E','F','I','\\',
        'N','O','V','O','S','\\',
        'N','O','V','O','S','_',
        'k','e','r','n','e','l',
        '.','e','l','f',
        0
    };

    EFI_FILE_PROTOCOL* kernelFile = nullptr;

    status = root->Open(
        root,
        &kernelFile,
        kernelPath,
        EFI_FILE_MODE_READ,
        0
    );

    if (status != EFI_SUCCESS || kernelFile == nullptr)
    {
        halt();
    }

    Elf64Header elfHeader;

    UINTN elfHeaderSize = sizeof(Elf64Header);

    status = kernelFile->Read(
        kernelFile,
        &elfHeaderSize,
        &elfHeader
    );

    if (status != EFI_SUCCESS ||
        elfHeaderSize != sizeof(Elf64Header))
    {
        halt();
    }

    if (elfHeader.e_ident[0] != 0x7F ||
        elfHeader.e_ident[1] != 'E' ||
        elfHeader.e_ident[2] != 'L' ||
        elfHeader.e_ident[3] != 'F')
    {
        halt();
    }

    if (elfHeader.e_ident[4] != ELF_CLASS_64 ||
        elfHeader.e_ident[5] != ELF_DATA_LSB)
    {
        halt();
    }

    if (elfHeader.e_machine != ELF_EM_X86_64 ||
        elfHeader.e_type != ELF_ET_EXEC)
    {
        halt();
    }

    if (elfHeader.e_phentsize != sizeof(Elf64ProgramHeader))
    {
        halt();
    }

    if (elfHeader.e_phnum == 0)
    {
        halt();
    }

    if (elfHeader.e_phoff == 0)
    {
        halt();
    }

    auto SetPosition =
        reinterpret_cast<EFI_FILE_SET_POSITION>(kernelFile->SetPosition);

    status = SetPosition(
        kernelFile,
        elfHeader.e_phoff
    );

    if (status != EFI_SUCCESS)
    {
        halt();
    }

    Elf64ProgramHeader programHeader;
    UINT16 loadSegmentCount = 0;

    for (UINT16 i = 0; i < elfHeader.e_phnum; ++i)
    {
        UINTN programHeaderSize = sizeof(Elf64ProgramHeader);

        status = kernelFile->Read(
            kernelFile,
            &programHeaderSize,
            &programHeader
        );

        if (status != EFI_SUCCESS ||
            programHeaderSize != sizeof(Elf64ProgramHeader))
        {
            halt();
        }

        if (programHeader.p_type == ELF_PT_LOAD)
        {
            ++loadSegmentCount;

            if (programHeader.p_memsz < programHeader.p_filesz)
            {
                halt();
            }

            if (programHeader.p_vaddr == 0)
            {
                halt();
            }

            if (programHeader.p_memsz == 0)
            {
                halt();
            }

            if (programHeader.p_offset > elfHeader.e_phoff &&
                programHeader.p_offset < elfHeader.e_phoff +
                                          static_cast<UINT64>(elfHeader.e_phnum) *
                                          sizeof(Elf64ProgramHeader))
            {
                halt();
            }

            if (programHeader.p_memsz >
                0xFFFFFFFFFFFFFFFFULL - programHeader.p_vaddr)
            {
                halt();
            }

            UINT64 segmentEnd =
                programHeader.p_vaddr + programHeader.p_memsz;

            UINT64 firstPage =
                programHeader.p_vaddr & ~0xFFFULL;

            UINT64 lastPage =
                (segmentEnd + 0xFFFULL) & ~0xFFFULL;

            if (lastPage < segmentEnd)
            {
                halt();
            }

            UINT64 pageCount =
                (lastPage - firstPage) / 0x1000ULL;

            if (pageCount == 0)
            {
                halt();
            }

            UINT64 allocationAddress = firstPage;

            status = AllocatePages(
                EFI_ALLOCATE_ADDRESS,
                EFI_LOADER_DATA,
                static_cast<UINTN>(pageCount),
                &allocationAddress
            );

            if (status != EFI_SUCCESS ||
                allocationAddress != firstPage)
            {
                halt();
            }
        }
    }

    if (loadSegmentCount == 0)
    {
        halt();
    }

    /*
     * Second pass:
     * reload the Program Header table and copy each PT_LOAD
     * from the ELF file into its allocated memory address.
     */

    status = SetPosition(
        kernelFile,
        elfHeader.e_phoff
    );

    if (status != EFI_SUCCESS)
    {
        halt();
    }

    for (UINT16 i = 0; i < elfHeader.e_phnum; ++i)
    {
        status = SetPosition(
            kernelFile,
            elfHeader.e_phoff +
                static_cast<UINT64>(i) * elfHeader.e_phentsize
        );

        if (status != EFI_SUCCESS)
        {
            halt();
        }

        UINTN programHeaderSize = sizeof(Elf64ProgramHeader);

        status = kernelFile->Read(
            kernelFile,
            &programHeaderSize,
            &programHeader
        );

        if (status != EFI_SUCCESS ||
            programHeaderSize != sizeof(Elf64ProgramHeader))
        {
            halt();
        }

        if (programHeader.p_type != ELF_PT_LOAD)
        {
            continue;
        }

        if (programHeader.p_filesz == 0)
        {
            continue;
        }

        if (programHeader.p_filesz >
            0xFFFFFFFFFFFFFFFFULL - programHeader.p_offset)
        {
            halt();
        }

        status = SetPosition(
            kernelFile,
            programHeader.p_offset
        );

        if (status != EFI_SUCCESS)
        {
            halt();
        }

        UINTN segmentSize =
            static_cast<UINTN>(programHeader.p_filesz);

        if (static_cast<UINT64>(segmentSize) !=
            programHeader.p_filesz)
        {
            halt();
        }

        void* segmentAddress =
            reinterpret_cast<void*>(programHeader.p_vaddr);

        status = kernelFile->Read(
            kernelFile,
            &segmentSize,
            segmentAddress
        );

        if (status != EFI_SUCCESS ||
            static_cast<UINT64>(segmentSize) !=
                programHeader.p_filesz)
        {
            halt();
        }
    }

    print(SystemTable, "NOVOS PT_LOAD LOADED\\r\\n");

    status = SetPosition(
        kernelFile,
        elfHeader.e_phoff
    );

    if (status != EFI_SUCCESS)
    {
        halt();
    }

    for (UINT16 i = 0; i < elfHeader.e_phnum; ++i)
    {
        status = SetPosition(
            kernelFile,
            elfHeader.e_phoff +
                static_cast<UINT64>(i) * elfHeader.e_phentsize
        );

        if (status != EFI_SUCCESS)
        {
            halt();
        }

        UINTN programHeaderSize = sizeof(Elf64ProgramHeader);

        status = kernelFile->Read(
            kernelFile,
            &programHeaderSize,
            &programHeader
        );

        if (status != EFI_SUCCESS ||
            programHeaderSize != sizeof(Elf64ProgramHeader))
        {
            halt();
        }

        if (programHeader.p_type != ELF_PT_LOAD ||
            programHeader.p_filesz == 0)
        {
            continue;
        }

        UINT8 fileByte = 0;

        status = SetPosition(
            kernelFile,
            programHeader.p_offset
        );

        if (status != EFI_SUCCESS)
        {
            halt();
        }

        UINTN byteSize = 1;

        status = kernelFile->Read(
            kernelFile,
            &byteSize,
            &fileByte
        );

        if (status != EFI_SUCCESS ||
            byteSize != 1)
        {
            halt();
        }

        volatile UINT8* loadedByte =
            reinterpret_cast<volatile UINT8*>(
                programHeader.p_vaddr
            );

        if (*loadedByte != fileByte)
        {
            halt();
        }
    }

    print(SystemTable, "NOVOS PT_LOAD VERIFIED\r\n");

    print(SystemTable, "NOVOS MEMORY MAP START\r\n");

    auto GetMemoryMap =
        reinterpret_cast<EFI_GET_MEMORY_MAP>(bs->GetMemoryMap);

    auto AllocatePool =
        reinterpret_cast<EFI_ALLOCATE_POOL>(bs->AllocatePool);

    auto ExitBootServices =
        reinterpret_cast<EFI_EXIT_BOOT_SERVICES>(bs->ExitBootServices);

    if (GetMemoryMap == nullptr ||
        AllocatePool == nullptr ||
        ExitBootServices == nullptr)
    {
        print(SystemTable, "MEMMAP FUNCTION PTR FAIL\r\n");
        halt();
    }

    UINTN memoryMapSize = 0;
    UINT64 memoryMapKey = 0;
    UINTN descriptorSize = 0;
    UINT32 descriptorVersion = 0;

    status = GetMemoryMap(
        &memoryMapSize,
        nullptr,
        &memoryMapKey,
        &descriptorSize,
        &descriptorVersion
    );

    if (status != EFI_BUFFER_TOO_SMALL ||
        memoryMapSize == 0 ||
        descriptorSize == 0)
    {
        print(SystemTable, "MEMMAP SIZE QUERY FAIL\r\n");
        halt();
    }

    UINTN memoryMapBufferSize =
        memoryMapSize + (descriptorSize * 8);

    void* memoryMapBuffer = nullptr;

    status = AllocatePool(
        EFI_LOADER_DATA,
        memoryMapBufferSize,
        &memoryMapBuffer
    );

    if (status != EFI_SUCCESS ||
        memoryMapBuffer == nullptr)
    {
        print(SystemTable, "MEMMAP ALLOC FAIL\r\n");
        halt();
    }

    print(SystemTable, "MEMMAP BUFFER OK\r\n");
    debug_str("A\n");

    /*
     * Allocate BootInfo structure
     * Must be a full page to remain valid after ExitBootServices
     */
    UINT64 bootInfoAddress = 0x1000;  /* Allocate at 0x1000 (second page) */
    status = AllocatePages(
        EFI_ALLOCATE_ADDRESS,
        EFI_LOADER_DATA,
        1,  /* 1 page */
        &bootInfoAddress
    );

    if (status != EFI_SUCCESS)
    {
        print(SystemTable, "BOOTINFO ALLOC FAIL\r\n");
        halt();
    }

    BootInfo* bootInfo = reinterpret_cast<BootInfo*>(bootInfoAddress);
    debug_str("BOOTINFO ALLOCATED AT: ");
    debug_hex64(reinterpret_cast<UINT64>(bootInfo));
    debug_str("\n");

    int retryCount = 0;

    for (;;)
    {
        debug_str("B\n");

        memoryMapSize = memoryMapBufferSize;

        status = GetMemoryMap(
            &memoryMapSize,
            reinterpret_cast<EFI_MEMORY_DESCRIPTOR*>(memoryMapBuffer),
            &memoryMapKey,
            &descriptorSize,
            &descriptorVersion
        );

        debug_str("C ");
        debug_hex64(status);
        debug_str("\n");

        if (status == EFI_BUFFER_TOO_SMALL)
        {
            print(SystemTable, "MEMMAP BUFFER TOO SMALL\r\n");
            halt();
        }

        if (status != EFI_SUCCESS)
        {
            print(SystemTable, "MEMMAP READ FAIL\r\n");
            halt();
        }

        if (descriptorSize < sizeof(EFI_MEMORY_DESCRIPTOR))
        {
            print(SystemTable, "MEMMAP DESC SIZE FAIL\r\n");
            halt();
        }

        if (descriptorVersion != EFI_MEMORY_DESCRIPTOR_VERSION)
        {
            print(SystemTable, "MEMMAP VERSION FAIL\r\n");
            halt();
        }

        debug_str("D\n");

        /*
         * Fill BootInfo before ExitBootServices
         */
        bootInfo->magic = NOVOS_BOOT_INFO_MAGIC;
        bootInfo->version = NOVOS_BOOT_INFO_VERSION;
        bootInfo->memory_map_address = reinterpret_cast<UINT64>(memoryMapBuffer);
        bootInfo->memory_map_size = memoryMapSize;
        bootInfo->memory_descriptor_size = descriptorSize;
        bootInfo->memory_descriptor_count = memoryMapSize / descriptorSize;

        debug_str("BOOTINFO FILLED\n");
        debug_str("  magic: ");
        debug_hex64(bootInfo->magic);
        debug_str("\n");
        debug_str("  memory_map_address: ");
        debug_hex64(bootInfo->memory_map_address);
        debug_str("\n");
        debug_str("  memory_map_size: ");
        debug_hex64(bootInfo->memory_map_size);
        debug_str("\n");
        debug_str("  descriptor_size: ");
        debug_hex64(bootInfo->memory_descriptor_size);
        debug_str("\n");
        debug_str("  descriptor_count: ");
        debug_hex64(bootInfo->memory_descriptor_count);
        debug_str("\n");

        status = ExitBootServices(
            ImageHandle,
            memoryMapKey
        );

        debug_str("E ");
        debug_hex64(status);
        debug_str("\n");

        if (status == EFI_SUCCESS)
        {
            break;
        }

        if (status != EFI_INVALID_PARAMETER)
        {
            print(SystemTable, "EXIT BOOT SERVICES FAIL\r\n");
            halt();
        }

        ++retryCount;

        if (retryCount > 10)
        {
            debug_str("RETRY LIMIT HIT\n");
            halt();
        }
    }

    debug_str("F\n");
    debug_str("JUMPING TO KERNEL\n");

    jump_to_kernel_with_bootinfo(elfHeader.e_entry, bootInfo);

    halt();
}
