#include "openfs.hpp"

static constexpr uint32_t OPENFS_INODE_COUNT = 128;
static constexpr uint32_t OPENFS_BITMAP_BLOCK = 1;
static constexpr uint32_t OPENFS_INODE_BLOCK = 2;
static constexpr uint32_t OPENFS_INODE_BLOCKS = 4;
static constexpr uint32_t OPENFS_DATA_BLOCK = OPENFS_INODE_BLOCK + OPENFS_INODE_BLOCKS;
static constexpr uint32_t OPENFS_DIRECT_COUNT = 8;
static constexpr uint32_t OPENFS_MAX_FILE = OPENFS_DIRECT_COUNT * OPENFS_BLOCK_SIZE;

struct OpenFsSuperblock {
    uint32_t magic, version, block_size, inode_size, inode_count;
    uint64_t total_blocks, bitmap_block, inode_table_block, data_block;
    uint64_t generation;
    uint8_t uuid[16];
    uint8_t reserved[440];
} __attribute__((packed));

static const BlockDevice* mounted = nullptr;

static bool io_read(uint64_t block, void* buffer) {
    if (!mounted || mounted->sector_size == 0 || OPENFS_BLOCK_SIZE % mounted->sector_size) return false;
    return block_read(mounted, block * (OPENFS_BLOCK_SIZE / mounted->sector_size),
                      OPENFS_BLOCK_SIZE / mounted->sector_size, buffer);
}
static bool io_write(uint64_t block, const void* buffer) {
    if (!mounted || mounted->sector_size == 0 || OPENFS_BLOCK_SIZE % mounted->sector_size) return false;
    return block_write(mounted, block * (OPENFS_BLOCK_SIZE / mounted->sector_size),
                       OPENFS_BLOCK_SIZE / mounted->sector_size, buffer);
}
static bool bitmap_get(uint64_t block) {
    uint8_t map[OPENFS_BLOCK_SIZE]={}; if(!io_read(OPENFS_BITMAP_BLOCK,map)) return false;
    return (map[block>>3] & (1U<<(block&7))) != 0;
}
static bool bitmap_set(uint64_t block,bool used) {
    uint8_t map[OPENFS_BLOCK_SIZE]={}; if(!io_read(OPENFS_BITMAP_BLOCK,map)) return false;
    if(used) map[block>>3]|=1U<<(block&7); else map[block>>3]&=~(1U<<(block&7));
    return io_write(OPENFS_BITMAP_BLOCK,map);
}
static bool inode_read(uint32_t n,OpenFsInode* inode) {
    if(!inode||n>=OPENFS_INODE_COUNT) return false;
    uint8_t block[OPENFS_BLOCK_SIZE]={};
    const uint32_t per=OPENFS_BLOCK_SIZE/OPENFS_INODE_SIZE;
    if(!io_read(OPENFS_INODE_BLOCK+n/per,block)) return false;
    *inode=*reinterpret_cast<const OpenFsInode*>(block+(n%per)*OPENFS_INODE_SIZE);
    return true;
}
static bool inode_write(uint32_t n,const OpenFsInode* inode) {
    if(!inode||n>=OPENFS_INODE_COUNT) return false;
    uint8_t block[OPENFS_BLOCK_SIZE]={}; const uint32_t per=OPENFS_BLOCK_SIZE/OPENFS_INODE_SIZE;
    if(!io_read(OPENFS_INODE_BLOCK+n/per,block)) return false;
    *reinterpret_cast<OpenFsInode*>(block+(n%per)*OPENFS_INODE_SIZE)=*inode;
    return io_write(OPENFS_INODE_BLOCK+n/per,block);
}
static bool find_file(const char* name,uint32_t* found) {
    if(!name||!found||!mounted) return false;
    uint8_t block[OPENFS_BLOCK_SIZE]={};
    OpenFsInode inode{};
    for(uint32_t i=1;i<OPENFS_INODE_COUNT;++i) {
        if(!inode_read(i,&inode)||inode.mode==0) continue;
        uint32_t block_index=(i*OPENFS_NAME_SIZE)/OPENFS_BLOCK_SIZE;
        (void)block_index;
        if(inode.direct[0]==0) continue;
        if(!io_read(inode.direct[0],block)) return false;
        const char* stored=reinterpret_cast<const char*>(block);
        bool equal=true; for(uint32_t j=0;j<OPENFS_NAME_SIZE;++j) if(stored[j]!=name[j]) {equal=false;break;}
        if(equal){*found=i;return true;}
    }
    return false;
}
extern "C" bool openfs_format(const BlockDevice* device) {
    if(!device||device->sector_size==0||OPENFS_BLOCK_SIZE%device->sector_size||device->sector_count<64) return false;
    mounted=device;
    const uint64_t blocks=(device->sector_count*device->sector_size)/OPENFS_BLOCK_SIZE;
    OpenFsSuperblock sb{}; sb.magic=OPENFS_MAGIC;sb.version=OPENFS_VERSION;sb.block_size=OPENFS_BLOCK_SIZE;
    sb.inode_size=OPENFS_INODE_SIZE;sb.inode_count=OPENFS_INODE_COUNT;sb.total_blocks=blocks;
    sb.bitmap_block=OPENFS_BITMAP_BLOCK;sb.inode_table_block=OPENFS_INODE_BLOCK;sb.data_block=OPENFS_DATA_BLOCK;sb.generation=1;
    uint8_t sector[OPENFS_BLOCK_SIZE]={}; *reinterpret_cast<OpenFsSuperblock*>(sector)=sb;
    if(!io_write(0,sector)) return false;
    for(uint32_t b=1;b<OPENFS_DATA_BLOCK;++b) if(!bitmap_set(b,true)) return false;
    OpenFsInode root{};root.mode=2;root.links=1;
    return inode_write(0,&root);
}
extern "C" bool openfs_mount(const BlockDevice* device) {
    if(!device||device->sector_size==0||OPENFS_BLOCK_SIZE%device->sector_size) return false;
    mounted=device; uint8_t block[OPENFS_BLOCK_SIZE]={}; if(!io_read(0,block)) return false;
    const auto* sb=reinterpret_cast<const OpenFsSuperblock*>(block);
    return sb->magic==OPENFS_MAGIC&&sb->version==OPENFS_VERSION&&sb->block_size==OPENFS_BLOCK_SIZE&&
           sb->inode_size==OPENFS_INODE_SIZE&&sb->inode_count==OPENFS_INODE_COUNT&&sb->data_block==OPENFS_DATA_BLOCK;
}
extern "C" bool openfs_create(const char* name,uint32_t mode) {
    if(!mounted||!name||!name[0]||find_file(name,&mode)) return false;
    uint32_t inode_no=0; OpenFsInode inode{};
    for(uint32_t i=1;i<OPENFS_INODE_COUNT;++i) if(inode_read(i,&inode)&&inode.mode==0){inode_no=i;break;}
    if(!inode_no) return false;
    uint64_t blocks=(mounted->sector_count*mounted->sector_size)/OPENFS_BLOCK_SIZE;
    uint64_t data_block=OPENFS_DATA_BLOCK;
    for(;data_block<blocks;++data_block) if(!bitmap_get(data_block)){if(!bitmap_set(data_block,true))return false;break;}
    if(data_block>=blocks) return false;
    inode={};inode.mode=mode;inode.links=1;inode.direct[0]=data_block;
    uint8_t block[OPENFS_BLOCK_SIZE]={}; for(uint32_t i=0;i<OPENFS_NAME_SIZE&&name[i];++i)block[i]=static_cast<uint8_t>(name[i]);
    if(!io_write(data_block,block)||!inode_write(inode_no,&inode)){bitmap_set(data_block,false);return false;}
    return true;
}
extern "C" bool openfs_write(const char* name,uint64_t offset,const void* data,uint64_t size) {
    uint32_t n=0;if(!data||!find_file(name,&n)||size>OPENFS_MAX_FILE-offset)return false;
    OpenFsInode inode{};if(!inode_read(n,&inode)||!inode.direct[0])return false;
    uint8_t block[OPENFS_BLOCK_SIZE]={};if(!io_read(inode.direct[0],block))return false;
    const auto* src=static_cast<const uint8_t*>(data);for(uint64_t i=0;i<size;++i)block[offset+i]=src[i];
    if(!io_write(inode.direct[0],block))return false; if(offset+size>inode.size)inode.size=offset+size;return inode_write(n,&inode);
}
extern "C" bool openfs_read(const char* name,uint64_t offset,void* data,uint64_t size,uint64_t* read_size) {
    if(read_size)*read_size=0;uint32_t n=0;if(!data||!find_file(name,&n))return false;OpenFsInode inode{};
    if(!inode_read(n,&inode)||offset>inode.size||!inode.direct[0])return false;uint8_t block[OPENFS_BLOCK_SIZE]={};
    if(!io_read(inode.direct[0],block))return false;uint64_t count=size<inode.size-offset?size:inode.size-offset;
    for(uint64_t i=0;i<count;++i)static_cast<uint8_t*>(data)[i]=block[offset+i];if(read_size)*read_size=count;return true;
}
extern "C" bool openfs_test() {
    static uint8_t disk[512*4096]={}; struct C{uint8_t* p;}; static C c{disk};
    const auto rd=[](const BlockDevice*d,uint64_t l,uint32_t n,void*o)->bool{auto*c=static_cast<C*>(d->context);if(l>=4096||!n||l+n>4096)return false;for(uint64_t i=0;i<uint64_t(n)*512;++i)static_cast<uint8_t*>(o)[i]=c->p[l*512+i];return true;};
    const auto wr=[](const BlockDevice*d,uint64_t l,uint32_t n,const void*i)->bool{auto*c=static_cast<C*>(d->context);if(l>=4096||!n||l+n>4096)return false;for(uint64_t x=0;x<uint64_t(n)*512;++x)c->p[l*512+x]=static_cast<const uint8_t*>(i)[x];return true;};
    BlockDevice d{0,BLOCK_DEVICE_MEMORY,512,4096,rd,wr,&c}; if(!openfs_format(&d)||!openfs_mount(&d)||!openfs_create("hello",1))return false;
    static const char msg[]="OpenFS";char out[sizeof(msg)]={};uint64_t n=0;
    return openfs_write("hello",0,msg,sizeof(msg))&&openfs_read("hello",0,out,sizeof(out),&n)&&n==sizeof(msg)&&
           out[0]=='O'&&out[1]=='p'&&out[2]=='e'&&out[3]=='n'&&out[4]=='F'&&out[5]=='S'&&out[6]==0;
}
