#include "openfs_adapter.hpp"
#include "openfs/format.h"
#include "openfs/mount.h"
#include "openfs/path.h"
#include "openfs/file.h"
#include "openfs/fsck.h"
#include "openfs/inode.h"
#include "../drivers/block.hpp"

struct OpenFsTestWindow
{
    const BlockDevice* base;
};

static bool openfs_test_window_read(const BlockDevice* device, uint64_t lba,
                                    uint32_t count, void* out)
{
    if (!device || !out || !device->context || !count ||
        lba >= device->sector_count ||
        static_cast<uint64_t>(count) > device->sector_count - lba)
        return false;
    const auto* window = static_cast<const OpenFsTestWindow*>(device->context);
    return window->base && block_read(window->base, lba, count, out);
}

static bool openfs_test_window_write(const BlockDevice* device, uint64_t lba,
                                     uint32_t count, const void* input)
{
    if (!device || !input || !device->context || !count ||
        lba >= device->sector_count ||
        static_cast<uint64_t>(count) > device->sector_count - lba)
        return false;
    const auto* window = static_cast<const OpenFsTestWindow*>(device->context);
    return window->base && block_write(window->base, lba, count, input);
}

static bool openfs_test_window_flush(const BlockDevice* device)
{
    if (!device || !device->context)
        return false;
    const auto* window = static_cast<const OpenFsTestWindow*>(device->context);
    return window->base && block_flush(window->base);
}

static void openfs_test_debug(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        asm volatile("outb %0,%1" : : "a"(s[i]), "Nd"(static_cast<unsigned short>(0xE9)));
}

extern "C" uint32_t openfs_kernel_test()
{
    openfs_test_debug("OPENFS TEST: START\n");
    const BlockDevice* disk=block_get(1U);
    if(!disk)return 22U;

    openfs_block_device_t device{};
    openfs_block_device_t second_device{};
    if (openfs_kernel_attach(disk,0U,4095U,&device) ||
        openfs_kernel_attach(disk,0U,65537U,&device) ||
        openfs_kernel_attach(disk,disk->sector_count,4096U,&device))
        return 21U;
    if(!openfs_kernel_attach(disk,0U,4096U,&device))return 1U;
    openfs_test_debug("OPENFS TEST: ATTACH OK\n");
    if(!openfs_kernel_attach(&test_disk,0U,4096U,&second_device))
    {
        openfs_kernel_detach(&device);
        return 18U;
    }

    uint8_t first_probe[4096]={};
    uint8_t second_probe[4096]={};
    if(device.read(device.context,0U,1U,first_probe)!=OPENFS_IO_OK ||
       second_device.read(second_device.context,0U,1U,second_probe)!=OPENFS_IO_OK)
    {
        openfs_kernel_detach(&second_device);
        openfs_kernel_detach(&device);
        return 19U;
    }
    for(size_t i=0U;i<sizeof(first_probe);++i)
        if(first_probe[i]!=second_probe[i])
        {
            openfs_kernel_detach(&second_device);
            openfs_kernel_detach(&device);
            return 20U;
        }
    openfs_kernel_detach(&second_device);

    static const uint8_t uuid[16]={
        0x41,0x72,0x63,0x68,0x69,0x61,0x4f,0x53,
        0x4f,0x70,0x65,0x6e,0x46,0x53,0x00,0x01
    };
    openfs_test_debug("OPENFS TEST: FORMAT START\n");
    if(openfs_format(&device,uuid)!=OPENFS_FORMAT_OK)return 2U;
    openfs_test_debug("OPENFS TEST: FORMAT OK\n");

    openfs_mount_t mount{};
    openfs_test_debug("OPENFS TEST: MOUNT START\n");
    openfs_test_debug("OPENFS TEST: REMOUNT START\n");
    if(openfs_mount(&mount,&device)!=OPENFS_MOUNT_OK)return 3U;
    openfs_test_debug("OPENFS TEST: REMOUNT OK\n");

    openfs_test_debug("OPENFS TEST: MOUNT OK\n");
    uint64_t root=0U;
    if(openfs_path_lookup_follow(&device,&mount.superblock,"/",&root)!=OPENFS_PATH_OK||root!=1U)return 4U;

    uint64_t dir=0U;
    if(openfs_path_mkdir(&device,&mount.superblock,"/system",&dir)!=OPENFS_PATH_OK)return 5U;

    uint64_t file=0U;
    if(openfs_path_create(&device,&mount.superblock,"/system/hello",0100644U,&file)!=OPENFS_PATH_OK)return 6U;

    openfs_inode_t inode{};
    const uint64_t inode_count=(mount.superblock.inode_table_blocks*(uint64_t)mount.superblock.block_size)/OPENFS_INODE_SIZE;
    if(openfs_inode_read(&device,mount.superblock.inode_table_start,file,inode_count,&inode)!=OPENFS_INODE_OK)return 7U;

    static const char message[]="ArchiaOS OpenFS";
    if(openfs_file_write(&device,&mount.superblock,&inode,0U,message,sizeof(message))!=OPENFS_FILE_OK)return 8U;

    openfs_test_debug("OPENFS TEST: SYNC START\n");
    if(openfs_sync(&mount)!=OPENFS_MOUNT_OK)return 9U;
    openfs_test_debug("OPENFS TEST: SYNC OK\n");

    openfs_test_debug("OPENFS TEST: FSCK START\n");
    uint64_t errors=0U;
    if(openfs_fsck(&device,&mount.superblock,&errors)!=OPENFS_FSCK_OK||errors!=0U)return 10U;
    openfs_test_debug("OPENFS TEST: FSCK OK\n");

    if(openfs_unmount(&mount)!=OPENFS_MOUNT_OK)return 11U;
    openfs_test_debug("OPENFS TEST: UNMOUNT OK\n");

    if(openfs_mount(&mount,&device)!=OPENFS_MOUNT_OK)return 3U;

    if(openfs_path_lookup_follow(&device,&mount.superblock,"/system/hello",&file)!=OPENFS_PATH_OK)return 13U;
    if(openfs_inode_read(&device,mount.superblock.inode_table_start,file,inode_count,&inode)!=OPENFS_INODE_OK)return 7U;

    char buffer[sizeof(message)]={};
    size_t got=0U;
    if(openfs_file_read(&device,&mount.superblock,&inode,0U,buffer,sizeof(buffer),&got)!=OPENFS_FILE_OK)return 15U;
    if(got!=sizeof(message))return 16U;
    for(size_t i=0U;i<sizeof(message);++i)if(buffer[i]!=message[i])return 17U;

    openfs_test_debug("OPENFS TEST: FINAL UNMOUNT\n");
    if(openfs_unmount(&mount)!=OPENFS_MOUNT_OK)
    {
        openfs_kernel_detach(&device);
        return 11U;
    }
    openfs_kernel_detach(&device);
    openfs_test_debug("OPENFS TEST: PASS\n");
    return 0U;
}
