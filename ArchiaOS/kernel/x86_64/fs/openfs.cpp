#include "openfs.hpp"

static constexpr uint32_t OPENFS_INODE_COUNT=128;
static constexpr uint32_t OPENFS_BITMAP_BLOCK=1;
static constexpr uint32_t OPENFS_INODE_BLOCK=2;
static constexpr uint32_t OPENFS_DIRECTORY_BLOCK=6;
static constexpr uint32_t OPENFS_DATA_BLOCK=7;
static constexpr uint32_t OPENFS_DIRECTORY_ENTRIES=64;
static constexpr uint32_t OPENFS_DIRECT_COUNT=8;
static constexpr uint32_t OPENFS_MAX_FILE=OPENFS_DIRECT_COUNT*OPENFS_BLOCK_SIZE;

struct OpenFsSuperblock {
 uint32_t magic,version,block_size,inode_size,inode_count;
 uint64_t total_blocks,bitmap_block,inode_table_block,directory_block,data_block,generation;
 uint8_t uuid[16]; uint8_t reserved[428];
} __attribute__((packed));
struct OpenFsDirEntry { uint32_t inode,type,parent; char name[OPENFS_NAME_SIZE]; uint8_t reserved[4]; } __attribute__((packed));
static_assert(sizeof(OpenFsSuperblock)==512,"OpenFS superblock size");
static_assert(sizeof(OpenFsInode)==128,"OpenFS inode size");
static_assert(sizeof(OpenFsDirEntry)==64,"OpenFS directory entry size");
static const BlockDevice* mounted=nullptr;
static uint32_t openfs_test_stage=0;

static bool io_read(uint64_t block,void* buffer){if(!mounted||!buffer||!mounted->sector_size||OPENFS_BLOCK_SIZE%mounted->sector_size)return false;return block_read(mounted,block*(OPENFS_BLOCK_SIZE/mounted->sector_size),OPENFS_BLOCK_SIZE/mounted->sector_size,buffer);}
static bool io_write(uint64_t block,const void* buffer){if(!mounted||!buffer||!mounted->sector_size||OPENFS_BLOCK_SIZE%mounted->sector_size)return false;return block_write(mounted,block*(OPENFS_BLOCK_SIZE/mounted->sector_size),OPENFS_BLOCK_SIZE/mounted->sector_size,buffer);}
static bool bitmap_get(uint64_t b){uint8_t m[OPENFS_BLOCK_SIZE]={};if(b>=OPENFS_BLOCK_SIZE*8||!io_read(OPENFS_BITMAP_BLOCK,m))return false;return (m[b>>3]&(1U<<(b&7)))!=0;}
static bool bitmap_set(uint64_t b,bool used){uint8_t m[OPENFS_BLOCK_SIZE]={};if(b>=OPENFS_BLOCK_SIZE*8||!io_read(OPENFS_BITMAP_BLOCK,m))return false;if(used)m[b>>3]|=1U<<(b&7);else m[b>>3]&=static_cast<uint8_t>(~(1U<<(b&7)));return io_write(OPENFS_BITMAP_BLOCK,m);}
static bool inode_read(uint32_t n,OpenFsInode* out){if(!out||n>=OPENFS_INODE_COUNT)return false;uint8_t b[OPENFS_BLOCK_SIZE]={};uint32_t per=OPENFS_BLOCK_SIZE/OPENFS_INODE_SIZE;if(!io_read(OPENFS_INODE_BLOCK+n/per,b))return false;*out=*reinterpret_cast<const OpenFsInode*>(b+(n%per)*OPENFS_INODE_SIZE);return true;}
static bool inode_write(uint32_t n,const OpenFsInode* in){if(!in||n>=OPENFS_INODE_COUNT)return false;uint8_t b[OPENFS_BLOCK_SIZE]={};uint32_t per=OPENFS_BLOCK_SIZE/OPENFS_INODE_SIZE;if(!io_read(OPENFS_INODE_BLOCK+n/per,b))return false;*reinterpret_cast<OpenFsInode*>(b+(n%per)*OPENFS_INODE_SIZE)=*in;return io_write(OPENFS_INODE_BLOCK+n/per,b);}
static bool valid_name(const char* name){
 if(!name||!name[0])return false;
 uint32_t length=0;
 while(name[length]){
  if(length>=OPENFS_NAME_SIZE-1)return false;
  if(name[length]=='/'||name[length]=='\\')return false;
  ++length;
 }
 return length>0;
}
static bool name_equal(const char* a,const char* b){for(uint32_t i=0;i<OPENFS_NAME_SIZE;++i){if(a[i]!=b[i])return false;if(a[i]==0)return true;}return true;}
extern "C" bool openfs_format(const BlockDevice* device){
 if(!device||!device->sector_size||OPENFS_BLOCK_SIZE%device->sector_size)return false;
 uint64_t blocks=(device->sector_count*device->sector_size)/OPENFS_BLOCK_SIZE;
 if(blocks<OPENFS_DATA_BLOCK+8||blocks>OPENFS_BLOCK_SIZE*8)return false;
 mounted=device;
 uint8_t zero[OPENFS_BLOCK_SIZE]={};
 for(uint32_t b=0;b<OPENFS_DATA_BLOCK;++b)if(!io_write(b,zero))return false;
 OpenFsSuperblock sb{};
 sb.magic=OPENFS_MAGIC;sb.version=OPENFS_VERSION;sb.block_size=OPENFS_BLOCK_SIZE;
 sb.inode_size=OPENFS_INODE_SIZE;sb.inode_count=OPENFS_INODE_COUNT;sb.total_blocks=blocks;
 sb.bitmap_block=OPENFS_BITMAP_BLOCK;sb.inode_table_block=OPENFS_INODE_BLOCK;
 sb.directory_block=OPENFS_DIRECTORY_BLOCK;sb.data_block=OPENFS_DATA_BLOCK;sb.generation=1;
 uint8_t sb_block[OPENFS_BLOCK_SIZE]={};
 *reinterpret_cast<OpenFsSuperblock*>(sb_block)=sb;
 if(!io_write(0,sb_block))return false;
 for(uint32_t b=0;b<OPENFS_DATA_BLOCK;++b)if(!bitmap_set(b,true))return false;
 OpenFsInode root{};root.mode=2;root.links=1;
 return inode_write(0,&root);
}
extern "C" bool openfs_mount(const BlockDevice* device){
 if(!device||!device->sector_size||OPENFS_BLOCK_SIZE%device->sector_size)return false;
 mounted=device;
 uint8_t b[OPENFS_BLOCK_SIZE]={};
 if(!io_read(0,b))return false;
 const auto* sb=reinterpret_cast<const OpenFsSuperblock*>(b);
 return sb->magic==OPENFS_MAGIC&&sb->version==OPENFS_VERSION&&
  sb->block_size==OPENFS_BLOCK_SIZE&&sb->inode_size==OPENFS_INODE_SIZE&&
  sb->inode_count==OPENFS_INODE_COUNT&&sb->directory_block==OPENFS_DIRECTORY_BLOCK&&
  sb->data_block==OPENFS_DATA_BLOCK&&
  sb->total_blocks==(device->sector_count*device->sector_size)/OPENFS_BLOCK_SIZE;
}
static bool find_child(uint32_t parent,const char* name,uint32_t* inode_no,uint32_t* slot){
 if(!mounted||!name||!inode_no)return false;
 uint8_t b[OPENFS_BLOCK_SIZE]={};
 if(!io_read(OPENFS_DIRECTORY_BLOCK,b))return false;
 auto* e=reinterpret_cast<const OpenFsDirEntry*>(b);
 for(uint32_t i=0;i<OPENFS_DIRECTORY_ENTRIES;++i)
  if(e[i].inode&&e[i].parent==parent&&name_equal(e[i].name,name)){
   *inode_no=e[i].inode;
   if(slot)*slot=i;
   return true;
  }
 return false;
}
static bool resolve_path(const char* path,uint32_t* inode_no){
 if(!path||!path[0]||!inode_no)return false;
 uint32_t current=0;
 uint32_t start=path[0]=='/'?1:0;
 while(path[start]){
  char component[OPENFS_NAME_SIZE]={};
  uint32_t length=0;
  while(path[start]&&path[start]!='/'){
   if(length>=OPENFS_NAME_SIZE-1)return false;
   component[length++]=path[start++];
  }
  while(path[start]=='/')++start;
  if(length==0||!valid_name(component))return false;
  uint32_t next=0;
  if(!find_child(current,component,&next,nullptr))return false;
  current=next;
 }
 *inode_no=current;
 return true;
}
static bool find_file(const char* name,uint32_t* inode_no){
 return resolve_path(name,inode_no);
}
static bool split_parent_path(const char* path,uint32_t* parent,char* leaf){
 if(!path||!parent||!leaf||!path[0])return false;
 uint32_t length=0,last_slash=0;
 while(path[length]){
  if(length>=255)return false;
  if(path[length]=='/')last_slash=length;
  ++length;
 }
 uint32_t leaf_start=last_slash+1;
 if(leaf_start>=length)return false;
 uint32_t leaf_length=length-leaf_start;
 if(leaf_length>=OPENFS_NAME_SIZE)return false;
 for(uint32_t i=0;i<leaf_length;++i)leaf[i]=path[leaf_start+i];
 leaf[leaf_length]=0;
 if(!valid_name(leaf))return false;
 if(last_slash==0){*parent=0;return true;}
 char parent_path[256]={};
 for(uint32_t i=0;i<last_slash;++i)parent_path[i]=path[i];
 parent_path[last_slash]=0;
 if(parent_path[0]==0){*parent=0;return true;}
 return resolve_path(parent_path,parent);
}
static bool create_node(const char* path,uint32_t mode){
 if(!mounted||!path||!path[0])return false;
 uint32_t parent=0;char leaf[OPENFS_NAME_SIZE]={};
 if(!split_parent_path(path,&parent,leaf))return false;
 uint32_t existing=0;
 if(find_child(parent,leaf,&existing,nullptr))return false;
 uint32_t inode_no=0;OpenFsInode inode{};
 for(uint32_t i=1;i<OPENFS_INODE_COUNT;++i)
  if(inode_read(i,&inode)&&inode.mode==0){inode_no=i;break;}
 if(!inode_no)return false;
 inode={};inode.mode=mode;inode.links=1;
 uint8_t db[OPENFS_BLOCK_SIZE]={};
 if(!io_read(OPENFS_DIRECTORY_BLOCK,db))return false;
 auto* entries=reinterpret_cast<OpenFsDirEntry*>(db);
 uint32_t slot=OPENFS_DIRECTORY_ENTRIES;
 for(uint32_t i=0;i<OPENFS_DIRECTORY_ENTRIES;++i)
  if(!entries[i].inode){slot=i;break;}
 if(slot==OPENFS_DIRECTORY_ENTRIES)return false;
 OpenFsDirEntry entry{};
 entry.inode=inode_no;entry.type=mode;entry.parent=parent;
 for(uint32_t i=0;i<OPENFS_NAME_SIZE&&leaf[i];++i)entry.name[i]=leaf[i];
 entries[slot]=entry;
 if(!inode_write(inode_no,&inode)||!io_write(OPENFS_DIRECTORY_BLOCK,db)){
  inode={};inode_write(inode_no,&inode);return false;
 }
 uint8_t verify[OPENFS_BLOCK_SIZE]={};if(!io_read(OPENFS_DIRECTORY_BLOCK,verify))return false;auto* verified=reinterpret_cast<const OpenFsDirEntry*>(verify);if(!verified[slot].inode||verified[slot].parent!=parent||!name_equal(verified[slot].name,leaf))return false;
 return true;
}
extern "C" bool openfs_create(const char* name,uint32_t mode){
 if(!mounted||mode==0||mode==2)return false;
 return create_node(name,mode);
}
extern "C" bool openfs_mkdir(const char* path){
 if(!mounted)return false;
 return create_node(path,2);
}
extern "C" bool openfs_unlink(const char* name){
 uint32_t inode_no=0,slot=0;
 if(!name||!name[0])return false;
 if(!find_file(name,&inode_no)||inode_no==0)return false;
 uint32_t parent=0;char leaf[OPENFS_NAME_SIZE]={};
 if(!split_parent_path(name,&parent,leaf))return false;
 if(!find_child(parent,leaf,&inode_no,&slot))return false;
 uint8_t db[OPENFS_BLOCK_SIZE]={};
 if(!io_read(OPENFS_DIRECTORY_BLOCK,db))return false;
 auto* entries=reinterpret_cast<OpenFsDirEntry*>(db);
 OpenFsInode inode{};
 if(!inode_read(inode_no,&inode)||inode.mode==0||inode.mode==2)return false;
 for(uint32_t i=0;i<OPENFS_DIRECT_COUNT;++i)
  if(inode.direct[i]&&!bitmap_set(inode.direct[i],false))return false;
 inode={};
 if(!inode_write(inode_no,&inode))return false;
 entries[slot]={};
 return io_write(OPENFS_DIRECTORY_BLOCK,db);
}
static bool allocate_block(uint64_t* result){
 if(!result||!mounted)return false;
 uint64_t blocks=(mounted->sector_count*mounted->sector_size)/OPENFS_BLOCK_SIZE;
 for(uint64_t b=OPENFS_DATA_BLOCK;b<blocks;++b)
  if(!bitmap_get(b)){
   if(bitmap_set(b,true)){*result=b;return true;}
   return false;
  }
 return false;
}
extern "C" bool openfs_write(const char* name,uint64_t offset,const void* data,uint64_t size){
 openfs_test_stage=20;uint32_t n=0;if(!data||!find_file(name,&n)||offset>OPENFS_MAX_FILE||size>OPENFS_MAX_FILE-offset)return false;openfs_test_stage=21;OpenFsInode inode{};if(!inode_read(n,&inode)||inode.mode==2)return false;
 const auto* src=static_cast<const uint8_t*>(data);uint64_t pos=0;while(pos<size){uint32_t bi=static_cast<uint32_t>((offset+pos)/OPENFS_BLOCK_SIZE);uint32_t in=static_cast<uint32_t>((offset+pos)%OPENFS_BLOCK_SIZE);if(bi>=OPENFS_DIRECT_COUNT)return false;uint64_t block=inode.direct[bi];if(!block){openfs_test_stage=22;if(!allocate_block(&block))return false;inode.direct[bi]=block;}uint8_t b[OPENFS_BLOCK_SIZE]={};openfs_test_stage=23;if(!io_read(block,b))return false;uint64_t count=size-pos;if(count>OPENFS_BLOCK_SIZE-in)count=OPENFS_BLOCK_SIZE-in;for(uint64_t i=0;i<count;++i)b[in+i]=src[pos+i];openfs_test_stage=24;if(!io_write(block,b))return false;pos+=count;}if(offset+size>inode.size)inode.size=offset+size;openfs_test_stage=25;return inode_write(n,&inode);
}
extern "C" bool openfs_read(const char* name,uint64_t offset,void* data,uint64_t size,uint64_t* read_size){
 if(read_size)*read_size=0;uint32_t n=0;if(!data||!find_file(name,&n))return false;OpenFsInode inode{};if(!inode_read(n,&inode)||offset>inode.size)return false;uint64_t count=size;if(count>inode.size-offset)count=inode.size-offset;auto* dst=static_cast<uint8_t*>(data);uint64_t pos=0;while(pos<count){uint32_t bi=static_cast<uint32_t>((offset+pos)/OPENFS_BLOCK_SIZE);uint32_t in=static_cast<uint32_t>((offset+pos)%OPENFS_BLOCK_SIZE);if(bi>=OPENFS_DIRECT_COUNT||!inode.direct[bi])return false;uint8_t b[OPENFS_BLOCK_SIZE]={};if(!io_read(inode.direct[bi],b))return false;uint64_t nbytes=count-pos;if(nbytes>OPENFS_BLOCK_SIZE-in)nbytes=OPENFS_BLOCK_SIZE-in;for(uint64_t i=0;i<nbytes;++i)dst[pos+i]=b[in+i];pos+=nbytes;}if(read_size)*read_size=count;return true;
}
extern "C" uint32_t openfs_test_stage_get(){return openfs_test_stage;}
extern "C" bool openfs_test(){
 openfs_test_stage=1;
 static uint8_t disk[512*4096]={};struct C{uint8_t* p;};static C c{disk};
 const auto rd=[](const BlockDevice*d,uint64_t l,uint32_t n,void*o)->bool{auto*c=static_cast<C*>(d->context);if(l>=4096||!n||l+n>4096)return false;for(uint64_t i=0;i<uint64_t(n)*512;++i)static_cast<uint8_t*>(o)[i]=c->p[l*512+i];return true;};
 const auto wr=[](const BlockDevice*d,uint64_t l,uint32_t n,const void*i)->bool{auto*c=static_cast<C*>(d->context);if(l>=4096||!n||l+n>4096)return false;for(uint64_t x=0;x<uint64_t(n)*512;++x)c->p[l*512+x]=static_cast<const uint8_t*>(i)[x];return true;};
 BlockDevice d{0,BLOCK_DEVICE_MEMORY,512,4096,rd,wr,&c};
 openfs_test_stage=2;if(!openfs_format(&d))return false;
 openfs_test_stage=3;if(!openfs_mount(&d))return false;
 openfs_test_stage=4;if(!openfs_create("hello",1))return false;
 static const char msg[]="OpenFS persistent";char out[sizeof(msg)]={};uint64_t n=0;uint32_t diagnostic_inode=0;
 openfs_test_stage=18;uint8_t diagnostic_dir[OPENFS_BLOCK_SIZE]={};if(!io_read(OPENFS_DIRECTORY_BLOCK,diagnostic_dir))return false;auto* diagnostic_entries=reinterpret_cast<const OpenFsDirEntry*>(diagnostic_dir);bool found_raw=false;for(uint32_t i=0;i<OPENFS_DIRECTORY_ENTRIES;++i)if(diagnostic_entries[i].inode&&name_equal(diagnostic_entries[i].name,"hello")&&diagnostic_entries[i].parent==0){found_raw=true;break;}if(!found_raw)return false;if(!find_child(0,"hello",&diagnostic_inode,nullptr))return false;openfs_test_stage=19;if(!find_file("hello",&diagnostic_inode))return false;
 openfs_test_stage=5;if(!openfs_write("hello",0,msg,sizeof(msg)))return false;openfs_test_stage=6;if(!openfs_read("hello",0,out,sizeof(out),&n)||n!=sizeof(msg)||!name_equal(out,msg))return false;
 static uint8_t large[OPENFS_BLOCK_SIZE*3+37];static uint8_t check[sizeof(large)];
 for(uint32_t i=0;i<sizeof(large);++i)large[i]=static_cast<uint8_t>((i*37U)+11U);
 openfs_test_stage=7;if(!openfs_write("hello",123,large,sizeof(large)))return false;openfs_test_stage=8;if(!openfs_read("hello",123,check,sizeof(check),&n)||n!=sizeof(check))return false;
 for(uint32_t i=0;i<sizeof(check);++i)if(check[i]!=large[i])return false;
 openfs_test_stage=9;if(!openfs_unlink("hello")||openfs_read("hello",0,out,sizeof(out),&n)||!openfs_create("hello",1))return false;
 openfs_test_stage=10;if(!openfs_unlink("hello")||openfs_read("hello",0,out,sizeof(out),&n)||!openfs_create("hello",1))return false;
 openfs_test_stage=11;if(!openfs_mkdir("system")||!openfs_create("system/config",1)||!openfs_write("system/config",0,msg,sizeof(msg)))return false;
 openfs_test_stage=12;if(!openfs_read("system/config",0,out,sizeof(out),&n)||n!=sizeof(msg)||!name_equal(out,msg))return false;
 openfs_test_stage=13;return !openfs_create("this-name-is-intentionally-too-long-for-openfs-123456",1);
}