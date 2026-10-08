#include "block.hpp"

static constexpr unsigned int MAX_DEVICES=16;
static constexpr uint32_t SECTOR_SIZE=512;
static constexpr uint64_t SECTOR_COUNT=4096;

struct MemoryContext { uint8_t* storage; };
static BlockDevice devices[MAX_DEVICES]={};
static bool initialized=false;
static uint8_t storage[SECTOR_SIZE*SECTOR_COUNT]={};
static MemoryContext context{storage};

static bool memory_read(const BlockDevice* d,uint64_t lba,uint32_t count,void* out) {
    if (!d||!out||d->context!=&context||lba>=d->sector_count||count==0||static_cast<uint64_t>(count)>d->sector_count-lba) return false;
    const uint64_t bytes=static_cast<uint64_t>(count)*d->sector_size, off=lba*d->sector_size;
    for(uint64_t i=0;i<bytes;++i) static_cast<uint8_t*>(out)[i]=storage[off+i];
    return true;
}
static bool memory_write(const BlockDevice* d,uint64_t lba,uint32_t count,const void* in) {
    if (!d||!in||d->context!=&context||lba>=d->sector_count||count==0||static_cast<uint64_t>(count)>d->sector_count-lba) return false;
    const uint64_t bytes=static_cast<uint64_t>(count)*d->sector_size, off=lba*d->sector_size;
    for(uint64_t i=0;i<bytes;++i) storage[off+i]=static_cast<const uint8_t*>(in)[i];
    return true;
}
static bool memory_flush(const BlockDevice* d) {
    return d && d->context == &context;
}
extern "C" bool block_initialize() {
    /*
     * BlockDevice pointers are consumed by VFS/storage layers. Resetting the
     * registry after registration would silently invalidate their state.
     */
    if (initialized)
        return true;

    for(auto& d:devices) d={};
    initialized=true;
    return true;
}
extern "C" bool block_register(BlockDevice* d,uint32_t* id) {
    if(!initialized||!d||!id||!d->sector_size||!d->sector_count||!d->read||!d->write) return false;
    for(unsigned int i=0;i<MAX_DEVICES;++i) if(!devices[i].id) { devices[i]=*d; devices[i].id=i+1; *id=i+1; return true; }
    return false;
}
extern "C" const BlockDevice* block_get(uint32_t id) {
    if(!initialized||!id||id>MAX_DEVICES||!devices[id-1].id) return nullptr;
    return &devices[id-1];
}
extern "C" unsigned int block_device_count() {
    unsigned int n=0; for(const auto& d:devices) if(d.id) ++n; return n;
}
extern "C" bool block_read(const BlockDevice* d,uint64_t lba,uint32_t count,void* out) {
    if(!d||!out||!d->sector_size||lba>=d->sector_count||!count||static_cast<uint64_t>(count)>d->sector_count-lba) return false;
    return d->read(d,lba,count,out);
}
extern "C" bool block_write(const BlockDevice* d,uint64_t lba,uint32_t count,const void* in) {
    if(!d||!in||!d->sector_size||lba>=d->sector_count||!count||static_cast<uint64_t>(count)>d->sector_count-lba) return false;
    return d->write(d,lba,count,in);
}
extern "C" bool block_flush(const BlockDevice* d) {
    if (!d || !d->flush)
        return false;
    return d->flush(d);
}
extern "C" bool block_memory_test() {
    if(!block_initialize()) return false;
    for(auto& b:storage) b=0;
    BlockDevice d{0,BLOCK_DEVICE_MEMORY,SECTOR_SIZE,SECTOR_COUNT,memory_read,memory_write,memory_flush,&context};
    uint32_t id=0; if(!block_register(&d,&id)||id!=1) return false;
    uint8_t w[1024]={},r[1024]={};
    for(unsigned int i=0;i<sizeof(w);++i) w[i]=static_cast<uint8_t>(i^0x5A);
    const BlockDevice* p=block_get(id);
    if (block_initialize() != true || block_get(id) != p)
        return false;
    if(!p||!block_write(p,7,2,w)||!block_read(p,7,2,r)||!block_flush(p)) return false;
    for(unsigned int i=0;i<sizeof(w);++i) if(w[i]!=r[i]) return false;
    return !block_read(p,SECTOR_COUNT-1,2,r)&&block_device_count()==1;
}
