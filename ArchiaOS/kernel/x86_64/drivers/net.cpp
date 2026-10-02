#include "net.hpp"

static constexpr unsigned int NET_QUEUE = 32;
static NetPacket packets[NET_QUEUE] = {};
static unsigned int head = 0;
static unsigned int tail = 0;
static bool initialized = false;

extern "C" bool net_initialize()
{
    head = tail = 0;
    initialized = true;
    return true;
}

extern "C" bool net_loopback_send(const NetPacket* packet)
{
    if (!initialized || !packet || packet->length > sizeof(packet->payload))
        return false;

    const unsigned int next = (head + 1) % NET_QUEUE;
    if (next == tail)
        return false;

    packets[head] = *packet;
    head = next;
    return true;
}

extern "C" bool net_receive(NetPacket* packet)
{
    if (!initialized || !packet || tail == head)
        return false;
    *packet = packets[tail];
    tail = (tail + 1) % NET_QUEUE;
    return true;
}

extern "C" unsigned int net_pending()
{
    return (head + NET_QUEUE - tail) % NET_QUEUE;
}

extern "C" uint16_t net_ipv4_checksum(const void* data, uint16_t length)
{
    if (!data)
        return 0;

    const auto* bytes = reinterpret_cast<const uint8_t*>(data);
    uint32_t sum = 0;
    for (uint16_t i = 0; i + 1 < length; i += 2)
        sum += static_cast<uint16_t>(bytes[i] << 8 | bytes[i + 1]);
    if (length & 1)
        sum += static_cast<uint16_t>(bytes[length - 1] << 8);

    while (sum >> 16)
        sum = (sum & 0xFFFFU) + (sum >> 16);

    return static_cast<uint16_t>(~sum);
}

extern "C" bool net_test()
{
    if (!net_initialize())
        return false;

    uint8_t header[20] = {};
    header[0] = 0x45;
    header[8] = 64;
    header[9] = 1;
    const uint16_t checksum = net_ipv4_checksum(header, sizeof(header));

    NetPacket packet{};
    packet.source = 0x7F000001U;
    packet.destination = 0x7F000001U;
    packet.protocol = 17;
    packet.length = 4;
    packet.payload[0] = 1;
    packet.payload[1] = 2;
    packet.payload[2] = 3;
    packet.payload[3] = 4;

    if (!net_loopback_send(&packet) || net_pending() != 1)
        return false;

    NetPacket received{};
    return net_receive(&received) &&
           received.source == packet.source &&
           received.destination == packet.destination &&
           received.protocol == packet.protocol &&
           received.payload[3] == 4 &&
           checksum != 0;
}
