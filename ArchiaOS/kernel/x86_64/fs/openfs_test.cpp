#include "openfs_adapter.hpp"
#include "../../../third_party/OpenFS/OpenFS/include/openfs/format.h"
#include "../../../third_party/OpenFS/OpenFS/include/openfs/mount.h"
#include "../../../third_party/OpenFS/OpenFS/include/openfs/path.h"
#include "../../../third_party/OpenFS/OpenFS/include/openfs/file.h"
#include "../../../third_party/OpenFS/OpenFS/include/openfs/fsck.h"
#include "../../../third_party/OpenFS/OpenFS/include/openfs/inode.h"
#include "../drivers/block.hpp"

extern "C" bool openfs_kernel_test()
{
    const BlockDevice* disk=block_get(1U);
    if(!disk)return false;

    openfs_block_device_t device{};
    if(!openfs_kernel_attach(disk,0U,4096U,&device))return false;

    static const uint8_t uuid[16]={
        0x41,0x72,0x63,0x68,0x69,0x61,0x4f,0x53,
        0x4f,0x70,0x65,0x6e,0x46,0x53,0x00,0x01
    };
    if(openfs_format(&device,uuid)!=OPENFS_FORMAT_OK)return false;

    openfs_mount_t mount{};
    if(openfs_mount(&mount,&device)!=OPENFS_MOUNT_OK)return false;

    uint64_t root=0U;
    if(openfs_path_lookup_follow(&device,&mount.superblock,"/",&root)!=OPENFS_PATH_OK||root!=1U)return false;

    uint64_t dir=0U;
    if(openfs_path_mkdir(&device,&mount.superblock,"/system",&dir)!=OPENFS_PATH_OK)return false;

    uint64_t file=0U;
    if(openfs_path_create(&device,&mount.superblock,"/system/hello",0100644U,&file)!=OPENFS_PATH_OK)return false;

    openfs_inode_t inode{};
    const uint64_t inode_count=(mount.superblock.inode_table_blocks*(uint64_t)mount.superblock.block_size)/OPENFS_INODE_SIZE;
    if(openfs_inode_read(&device,mount.superblock.inode_table_start,inode_count,file,&inode)!=OPENFS_INODE_OK)return false;

    static const char message[]="ArchiaOS OpenFS";
    if(openfs_file_write(&device,&mount.superblock,&inode,0U,message,sizeof(message))!=OPENFS_FILE_OK)return false;

    if(openfs_sync(&mount)!=OPENFS_MOUNT_OK)return false;

    uint64_t errors=0U;
    if(openfs_fsck(&device,&mount.superblock,&errors)!=OPENFS_FSCK_OK||errors!=0U)return false;

    if(openfs_unmount(&mount)!=OPENFS_MOUNT_OK)return false;

    if(openfs_mount(&mount,&device)!=OPENFS_MOUNT_OK)return false;

    if(openfs_path_lookup_follow(&device,&mount.superblock,"/system/hello",&file)!=OPENFS_PATH_OK)return false;
    if(openfs_inode_read(&device,mount.superblock.inode_table_start,inode_count,file,&inode)!=OPENFS_INODE_OK)return false;

    char buffer[sizeof(message)]={};
    size_t got=0U;
    if(openfs_file_read(&device,&mount.superblock,&inode,0U,buffer,sizeof(buffer),&got)!=OPENFS_FILE_OK)return false;
    if(got!=sizeof(message))return false;
    for(size_t i=0U;i<sizeof(message);++i)if(buffer[i]!=message[i])return false;

    if(openfs_unmount(&mount)!=OPENFS_MOUNT_OK)return false;
    return true;
}
