#include "openfs_adapter.hpp"
#include "../memory/heap.hpp"

struct OpenFsKernelContext
{
    const BlockDevice* device;
    uint64_t first_lba;
    uint32_t sectors_per_block;
};

static openfs_io_result_t read_blocks(void* context,uint64_t first,uint32_t count,void* buffer)
{
    auto* c=static_cast<OpenFsKernelContext*>(context);
    if(!c||!c->device||!buffer||!count||c->sectors_per_block==0U)return OPENFS_IO_INVALID_ARGUMENT;
    if(first>UINT64_MAX/c->sectors_per_block)return OPENFS_IO_OUT_OF_RANGE;
    const uint64_t lba_offset=first*c->sectors_per_block;
    if(lba_offset>UINT64_MAX-c->first_lba)return OPENFS_IO_OUT_OF_RANGE;
    if((uint64_t)count>UINT32_MAX/c->sectors_per_block)return OPENFS_IO_OUT_OF_RANGE;
    const uint32_t sectors=count*c->sectors_per_block;
    if(!block_read(c->device,c->first_lba+lba_offset,sectors,buffer))return OPENFS_IO_IO_ERROR;
    return OPENFS_IO_OK;
}

static openfs_io_result_t write_blocks(void* context,uint64_t first,uint32_t count,const void* buffer)
{
    auto* c=static_cast<OpenFsKernelContext*>(context);
    if(!c||!c->device||!buffer||!count||c->sectors_per_block==0U)return OPENFS_IO_INVALID_ARGUMENT;
    if(first>UINT64_MAX/c->sectors_per_block)return OPENFS_IO_OUT_OF_RANGE;
    const uint64_t lba_offset=first*c->sectors_per_block;
    if(lba_offset>UINT64_MAX-c->first_lba)return OPENFS_IO_OUT_OF_RANGE;
    if((uint64_t)count>UINT32_MAX/c->sectors_per_block)return OPENFS_IO_OUT_OF_RANGE;
    const uint32_t sectors=count*c->sectors_per_block;
    if(!block_write(c->device,c->first_lba+lba_offset,sectors,buffer))return OPENFS_IO_IO_ERROR;
    return OPENFS_IO_OK;
}

static openfs_io_result_t flush_blocks(void* context)
{
    auto* c=static_cast<OpenFsKernelContext*>(context);
    return c&&c->device?OPENFS_IO_OK:OPENFS_IO_INVALID_ARGUMENT;
}

extern "C" bool openfs_kernel_attach(
    const BlockDevice* device,
    uint64_t first_lba,
    uint32_t openfs_block_size,
    openfs_block_device_t* out_device)
{
    static OpenFsKernelContext context{};
    if(!device||!out_device||openfs_block_size<4096U||device->sector_size==0U||
       openfs_block_size%device->sector_size!=0U||first_lba>=device->sector_count)return false;
    const uint32_t spb=openfs_block_size/device->sector_size;
    if(spb==0U||spb>UINT32_MAX)return false;
    const uint64_t available=device->sector_count-first_lba;
    const uint64_t blocks=available/spb;
    if(blocks<64U)return false;
    context={device,first_lba,spb};
    out_device->context=&context;
    out_device->block_size=openfs_block_size;
    out_device->block_count=blocks;
    out_device->read=read_blocks;
    out_device->write=write_blocks;
    out_device->flush=flush_blocks;
    return true;
}
