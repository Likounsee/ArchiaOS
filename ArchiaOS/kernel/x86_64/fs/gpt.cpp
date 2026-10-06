#include "gpt.hpp"

static constexpr uint32_t HEADER_SIZE=92, ENTRY_SIZE=128, MAX_ENTRIES=128;
struct GptHeader {
    uint8_t signature[8]; uint32_t revision,header_size,header_crc32,reserved;
    uint64_t current_lba,backup_lba,first_usable_lba,last_usable_lba;
    uint8_t disk_guid[16]; uint64_t partition_entries_lba;
    uint32_t partition_entry_count,partition_entry_size,partition_entries_crc32;
} __attribute__((packed));
struct GptEntry {
    uint8_t type_guid[16],unique_guid[16]; uint64_t first_lba,last_lba,attributes; uint16_t name[36];
} __attribute__((packed));

static uint32_t crc32(const uint8_t* p,uint32_t n) {
    uint32_t c=0xFFFFFFFFU;
    for(uint32_t i=0;i<n;++i){ c^=p[i]; for(unsigned int b=0;b<8;++b) c=(c>>1)^((c&1U)?0xEDB88320U:0); }
    return c^0xFFFFFFFFU;
}
static bool signature_ok(const uint8_t* s) {
    static constexpr uint8_t e[8]={'E','F','I',' ','P','A','R','T'};
    for(unsigned int i=0;i<8;++i) if(s[i]!=e[i]) return false; return true;
}
extern "C" bool gpt_read_partitions(const BlockDevice* d,GptPartition* out,uint32_t cap,uint32_t* count) {
    if(!d||!out||!count||!cap||d->sector_size<512) return false;
    *count=0; uint8_t sector[512]={};
    if(!block_read(d,1,1,sector)) return false;
    const auto* h=reinterpret_cast<const GptHeader*>(sector);
    if(!signature_ok(h->signature)||h->revision!=0x00010000U||h->header_size<HEADER_SIZE||h->header_size>d->sector_size||
       h->current_lba!=1||!h->partition_entry_count||h->partition_entry_size<ENTRY_SIZE||
       h->partition_entry_size>d->sector_size||h->partition_entries_lba>=d->sector_count) return false;
    uint8_t copy[512]={}; for(unsigned int i=0;i<512;++i) copy[i]=sector[i];
    reinterpret_cast<GptHeader*>(copy)->header_crc32=0;
    if(crc32(copy,h->header_size)!=h->header_crc32) return false;
    const uint64_t bytes=static_cast<uint64_t>(h->partition_entry_count)*h->partition_entry_size;
    const uint64_t sectors=(bytes+d->sector_size-1)/d->sector_size;
    if(!sectors||sectors>d->sector_count-h->partition_entries_lba||bytes>16384) return false;
    uint8_t entries[16384]={};
    if(!block_read(d,h->partition_entries_lba,static_cast<uint32_t>(sectors),entries)||
       crc32(entries,static_cast<uint32_t>(bytes))!=h->partition_entries_crc32) return false;
    const uint32_t scan=h->partition_entry_count<MAX_ENTRIES?h->partition_entry_count:MAX_ENTRIES;
    for(uint32_t i=0;i<scan&&*count<cap;++i) {
        const auto* e=reinterpret_cast<const GptEntry*>(entries+static_cast<uint64_t>(i)*h->partition_entry_size);
        bool empty=true; for(unsigned int b=0;b<16;++b) if(e->type_guid[b]) empty=false;
        if(empty) continue;
        if(e->first_lba>e->last_lba||e->last_lba>=d->sector_count||e->first_lba<h->first_usable_lba||e->last_lba>h->last_usable_lba) return false;
        auto& p=out[*count]; p.index=i+1;p.first_lba=e->first_lba;p.last_lba=e->last_lba;p.attributes=e->attributes;
        for(unsigned int b=0;b<16;++b){p.type_guid[b]=e->type_guid[b];p.unique_guid[b]=e->unique_guid[b];} ++*count;
    }
    return true;
}
extern "C" bool gpt_test() {
    constexpr uint32_t SS=512; constexpr uint64_t SC=4096;
    static uint8_t disk[SS*SC]={};
    struct Ctx{uint8_t* p;}; static Ctx ctx{disk};
    const auto read=[](const BlockDevice* d,uint64_t l,uint32_t n,void* o)->bool{
        const auto* c=static_cast<const Ctx*>(d->context); if(!c||!o||l>=SC||!n||static_cast<uint64_t>(n)>SC-l) return false;
        for(uint64_t i=0;i<static_cast<uint64_t>(n)*SS;++i) static_cast<uint8_t*>(o)[i]=c->p[l*SS+i]; return true; };
    const auto flush=[](const BlockDevice* d)->bool{return d&&d->context!=nullptr;};
    const auto write=[](const BlockDevice* d,uint64_t l,uint32_t n,const void* in)->bool{
        const auto* c=static_cast<const Ctx*>(d->context); if(!c||!in||l>=SC||!n||static_cast<uint64_t>(n)>SC-l) return false;
        for(uint64_t i=0;i<static_cast<uint64_t>(n)*SS;++i) c->p[l*SS+i]=static_cast<const uint8_t*>(in)[i]; return true; };
    for(auto& b:disk)b=0; BlockDevice d{0,BLOCK_DEVICE_MEMORY,SS,SC,read,write,flush,&ctx};
    GptHeader h{}; const uint8_t sig[8]={'E','F','I',' ','P','A','R','T'}; for(unsigned int i=0;i<8;++i)h.signature[i]=sig[i];
    h.revision=0x00010000U;h.header_size=HEADER_SIZE;h.current_lba=1;h.backup_lba=SC-1;h.first_usable_lba=34;h.last_usable_lba=SC-34;
    h.partition_entries_lba=2;h.partition_entry_count=MAX_ENTRIES;h.partition_entry_size=ENTRY_SIZE;
    GptEntry e{};e.type_guid[0]=0xAA;e.unique_guid[0]=0x55;e.first_lba=2048;e.last_lba=3071;e.attributes=1;
    for(unsigned int i=0;i<sizeof(e);++i)disk[2*SS+i]=reinterpret_cast<const uint8_t*>(&e)[i];
    h.partition_entries_crc32=crc32(disk+2*SS,MAX_ENTRIES*ENTRY_SIZE); h.header_crc32=0;
    h.header_crc32=crc32(reinterpret_cast<const uint8_t*>(&h),HEADER_SIZE);
    for(unsigned int i=0;i<sizeof(h);++i)disk[SS+i]=reinterpret_cast<const uint8_t*>(&h)[i];
    GptPartition p[4]={};uint32_t n=0;
    if(!gpt_read_partitions(&d,p,4,&n)||n!=1||p[0].index!=1||p[0].first_lba!=2048||p[0].last_lba!=3071) return false;
    disk[SS+24]^=1; return !gpt_read_partitions(&d,p,4,&n);
}
