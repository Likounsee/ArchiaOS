#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

extern void *kmalloc(uint64_t size);
extern void kfree(void *pointer);

void *malloc(size_t size) { return kmalloc((uint64_t)size); }
void *calloc(size_t count,size_t size)
{
    if(count!=0U && size>SIZE_MAX/count)return NULL;
    size_t total=count*size;
    void *p=malloc(total);
    if(p==NULL)return NULL;
    memset(p,0,total);
    return p;
}
void *realloc(void *ptr,size_t size)
{
    if(ptr==NULL)return malloc(size);
    if(size==0U){free(ptr);return NULL;}
    return NULL;
}
void free(void *ptr) { kfree(ptr); }

int memcmp(const void *a,const void *b,size_t n)
{
    const unsigned char *x=(const unsigned char*)a,*y=(const unsigned char*)b;
    for(size_t i=0;i<n;++i)if(x[i]!=y[i])return x[i]<y[i]?-1:1;
    return 0;
}
void *memchr(const void *s,int c,size_t n)
{
    const unsigned char *p=(const unsigned char*)s;
    for(size_t i=0;i<n;++i)if(p[i]==(unsigned char)c)return (void*)(p+i);
    return NULL;
}
size_t strlen(const char *s)
{
    size_t n=0U;
    if(s==NULL)return 0U;
    while(s[n]!='\0')++n;
    return n;
}
char *strchr(const char *s,int c)
{
    if(s==NULL)return NULL;
    for(;;++s){if(*s==(char)c)return (char*)s;if(*s=='\0')return NULL;}
}
char *strrchr(const char *s,int c)
{
    if(s==NULL)return NULL;
    const char *last=NULL;
    for(;;++s){if(*s==(char)c)last=s;if(*s=='\0')return (char*)last;}
}
int strcmp(const char *a,const char *b)
{
    while(*a&&*a==*b){++a;++b;}
    return (unsigned char)*a-(unsigned char)*b;
}
int strncmp(const char *a,const char *b,size_t n)
{
    for(size_t i=0;i<n;++i){unsigned char x=(unsigned char)a[i],y=(unsigned char)b[i];if(x!=y)return x<y?-1:1;if(x==0U)return 0;}
    return 0;
}

uint64_t openfs_time_now_ns(void)
{
    return UINT64_MAX;
}
