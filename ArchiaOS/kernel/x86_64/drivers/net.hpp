#pragma once

#include <stdint.h>

struct NetPacket
{
    uint32_t source;
    uint32_t destination;
    uint16_t protocol;
    uint16_t length;
    uint8_t payload[1500];
};

extern "C" bool net_initialize();
extern "C" bool net_loopback_send(const NetPacket* packet);
extern "C" bool net_receive(NetPacket* packet);
extern "C" unsigned int net_pending();
extern "C" uint16_t net_ipv4_checksum(const void* data, uint16_t length);
extern "C" bool net_test();
