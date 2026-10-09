#include "storage.hpp"
#include "block.hpp"
#include "../memory/pmm.hpp"
#include "../memory/paging.hpp"
#include "../fs/gpt.hpp"

static constexpr unsigned int STORAGE_MAX_CONTROLLERS = 8;
static StorageController controllers[STORAGE_MAX_CONTROLLERS] = {};
static unsigned int controller_count = 0;
static unsigned int real_block_device_count = 0;
static bool storage_initialized = false;

static constexpr uint32_t AHCI_GHC = 0x04;
static constexpr uint32_t AHCI_CAP = 0x00;
static constexpr uint32_t AHCI_PI = 0x0C;
static constexpr uint32_t AHCI_PORT_BASE = 0x100;
static constexpr uint32_t AHCI_PORT_STRIDE = 0x80;
static constexpr uint32_t AHCI_PxCLB = 0x00;
static constexpr uint32_t AHCI_PxCLBU = 0x04;
static constexpr uint32_t AHCI_PxFB = 0x08;
static constexpr uint32_t AHCI_PxFBU = 0x0C;
static constexpr uint32_t AHCI_PxIS = 0x10;
static constexpr uint32_t AHCI_PxIE = 0x14;
static constexpr uint32_t AHCI_PxCMD = 0x18;
static constexpr uint32_t AHCI_PxTFD = 0x20;
static constexpr uint32_t AHCI_PxSIG = 0x24;
static constexpr uint32_t AHCI_PxSSTS = 0x28;
static constexpr uint32_t AHCI_PxCI = 0x38;
static constexpr uint32_t AHCI_CMD_ST = 1U << 0;
static constexpr uint32_t AHCI_CMD_FRE = 1U << 4;
static constexpr uint32_t AHCI_CMD_FR = 1U << 14;
static constexpr uint32_t AHCI_CMD_CR = 1U << 15;
static constexpr uint32_t AHCI_TFD_BSY = 1U << 7;
static constexpr uint32_t AHCI_TFD_DRQ = 1U << 3;
static constexpr uint32_t AHCI_IS_TFES = 1U << 30;
static constexpr uint32_t AHCI_SIG_SATA = 0x00000101U;
static constexpr uint8_t ATA_FIS_REG_H2D = 0x27;
static constexpr uint8_t ATA_CMD_IDENTIFY = 0xEC;
static constexpr uint8_t ATA_CMD_READ_DMA_EXT = 0x25;
static constexpr uint8_t ATA_CMD_WRITE_DMA_EXT = 0x35;
static void storage_debug(const char* s)
{
    for (int i = 0; s[i] != '\0'; ++i)
        asm volatile("outb %0,%1"
                     : : "a"(s[i]),
                         "Nd"(static_cast<unsigned short>(0xE9)));
}

static constexpr uint32_t AHCI_TIMEOUT = 1000000U;
static constexpr uint32_t AHCI_MAX_SECTORS_PER_COMMAND = 16U;
static constexpr uint32_t AHCI_SECTOR_SIZE = 512U;

struct AhciFisH2D
{
    uint8_t type;
    uint8_t flags;
    uint8_t command;
    uint8_t feature_low;
    uint8_t lba0,lba1,lba2,device;
    uint8_t lba3,lba4,lba5,feature_high;
    uint8_t count_low,count_high,icc,control;
    uint8_t reserved[4];
} __attribute__((packed));

struct AhciCmdHeader
{
    uint16_t flags;
    uint16_t prdt_length;
    uint32_t byte_count;
    uint32_t command_table;
    uint32_t command_table_upper;
    uint32_t reserved[4];
} __attribute__((packed));

struct AhciPrdt
{
    uint32_t data_base;
    uint32_t data_base_upper;
    uint32_t reserved;
    uint32_t byte_count;
} __attribute__((packed));

struct AhciPortContext
{
    volatile uint8_t* regs;
    uint64_t command_list_physical;
    uint64_t fis_physical;
    uint64_t table_physical;
    uint64_t dma_physical;
    uint8_t* command_list;
    uint8_t* fis;
    uint8_t* table;
    uint8_t* dma;
    uint64_t sector_count;
    uint32_t block_id;
    bool initialized;
};

static AhciPortContext ahci_ports[32] = {};

static volatile uint32_t* ahci_reg(volatile uint8_t* base, uint32_t offset)
{
    return reinterpret_cast<volatile uint32_t*>(const_cast<uint8_t*>(base + offset));
}

static void zero_bytes(void* pointer, uint64_t count)
{
    auto* bytes = static_cast<uint8_t*>(pointer);
    for (uint64_t i = 0; i < count; ++i)
        bytes[i] = 0;
}

static void copy_bytes(void* destination, const void* source, uint64_t count)
{
    auto* dst = static_cast<uint8_t*>(destination);
    const auto* src = static_cast<const uint8_t*>(source);
    for (uint64_t i = 0; i < count; ++i)
        dst[i] = src[i];
}

static bool wait_clear(volatile uint32_t* reg, uint32_t mask)
{
    for (uint32_t i = 0; i < AHCI_TIMEOUT; ++i)
        if ((*reg & mask) == 0)
            return true;
    return false;
}

static bool ahci_stop(AhciPortContext& port)
{
    auto* cmd = ahci_reg(port.regs, AHCI_PxCMD);
    *cmd &= ~(AHCI_CMD_ST | AHCI_CMD_FRE);
    return wait_clear(cmd, AHCI_CMD_FR | AHCI_CMD_CR);
}

static bool ahci_start(AhciPortContext& port)
{
    auto* cmd = ahci_reg(port.regs, AHCI_PxCMD);
    if (!wait_clear(cmd, AHCI_CMD_CR))
        return false;
    *cmd |= AHCI_CMD_FRE | AHCI_CMD_ST;
    return true;
}

static void ahci_release_port(AhciPortContext& port)
{
    if (port.initialized && port.regs)
        (void)ahci_stop(port);

    if (port.command_list_physical)
        pmm_free_page(port.command_list_physical);
    if (port.fis_physical)
        pmm_free_page(port.fis_physical);
    if (port.table_physical)
        pmm_free_page(port.table_physical);
    if (port.dma_physical)
    {
        pmm_free_page(port.dma_physical);
        pmm_free_page(port.dma_physical + PAGE_SIZE);
    }

    port = {};
}


static int ahci_find_slot(AhciPortContext& port, uint32_t slot_count)
{
    const uint32_t used =
        *ahci_reg(port.regs, AHCI_PxCI) |
        *ahci_reg(port.regs, 0x34);
    for (uint32_t slot = 0; slot < slot_count; ++slot)
        if ((used & (1U << slot)) == 0)
            return static_cast<int>(slot);
    return -1;
}

static bool ahci_issue(AhciPortContext& port, uint8_t command,
                       uint64_t lba, uint32_t count, bool write,
                       void* buffer)
{
    if (!port.initialized || !buffer || !count ||
        count > AHCI_MAX_SECTORS_PER_COMMAND)
        return false;

    const uint32_t bytes = count * AHCI_SECTOR_SIZE;
    const int slot = ahci_find_slot(port, 32);
    if (slot < 0)
        return false;

    if (!wait_clear(ahci_reg(port.regs, AHCI_PxTFD), AHCI_TFD_BSY | AHCI_TFD_DRQ))
        return false;

    *ahci_reg(port.regs, AHCI_PxIS) = 0xFFFFFFFFU;

    auto* header = reinterpret_cast<AhciCmdHeader*>(
        port.command_list + static_cast<uint64_t>(slot) * sizeof(AhciCmdHeader));
    zero_bytes(header, sizeof(AhciCmdHeader));
    header->flags = static_cast<uint16_t>(5U | (write ? (1U << 6) : 0U));
    header->prdt_length = 1;
    header->command_table =
        static_cast<uint32_t>(port.table_physical & 0xFFFFFFFFULL);
    header->command_table_upper =
        static_cast<uint32_t>(port.table_physical >> 32);

    zero_bytes(port.table, 0x1000);
    auto* prdt = reinterpret_cast<AhciPrdt*>(port.table + 0x80);
    const uint64_t data_physical = port.dma_physical;
    prdt->data_base = static_cast<uint32_t>(data_physical);
    prdt->data_base_upper = static_cast<uint32_t>(data_physical >> 32);
    prdt->byte_count = (bytes - 1U) | (1U << 31);

    auto* fis = reinterpret_cast<AhciFisH2D*>(port.table);
    zero_bytes(fis, sizeof(AhciFisH2D));
    fis->type = ATA_FIS_REG_H2D;
    fis->flags = 0x80;
    fis->command = command;
    fis->device = 0x40;
    fis->lba0 = static_cast<uint8_t>(lba);
    fis->lba1 = static_cast<uint8_t>(lba >> 8);
    fis->lba2 = static_cast<uint8_t>(lba >> 16);
    fis->lba3 = static_cast<uint8_t>(lba >> 24);
    fis->lba4 = static_cast<uint8_t>(lba >> 32);
    fis->lba5 = static_cast<uint8_t>(lba >> 40);
    fis->count_low = static_cast<uint8_t>(count);
    fis->count_high = static_cast<uint8_t>(count >> 8);

    if (write)
        copy_bytes(port.dma, buffer, bytes);

    asm volatile("" ::: "memory");
    *ahci_reg(port.regs, AHCI_PxCI) = 1U << slot;

    for (uint32_t spin = 0; spin < AHCI_TIMEOUT; ++spin)
    {
        if ((*ahci_reg(port.regs, AHCI_PxIS) & AHCI_IS_TFES) != 0)
            return false;
        if ((*ahci_reg(port.regs, AHCI_PxCI) & (1U << slot)) == 0)
            break;
        if (spin + 1 == AHCI_TIMEOUT)
            return false;
    }

    if ((*ahci_reg(port.regs, AHCI_PxIS) & AHCI_IS_TFES) != 0)
        return false;

    if (!write)
        copy_bytes(buffer, port.dma, bytes);

    *ahci_reg(port.regs, AHCI_PxIS) = 0xFFFFFFFFU;
    return true;
}

static bool ahci_identify(AhciPortContext& port)
{
    zero_bytes(port.dma, AHCI_SECTOR_SIZE);
    if (!ahci_issue(port, ATA_CMD_IDENTIFY, 0, 1, false, port.dma))
        return false;

    const auto* identify = reinterpret_cast<const uint16_t*>(port.dma);
    uint64_t sectors =
        static_cast<uint64_t>(identify[100]) |
        (static_cast<uint64_t>(identify[101]) << 16) |
        (static_cast<uint64_t>(identify[102]) << 32) |
        (static_cast<uint64_t>(identify[103]) << 48);

    if (!sectors)
        sectors =
            static_cast<uint64_t>(identify[60]) |
            (static_cast<uint64_t>(identify[61]) << 16);

    if (!sectors)
        return false;

    port.sector_count = sectors;
    return true;
}

static bool ahci_read(const BlockDevice* device, uint64_t lba,
                      uint32_t count, void* out)
{
    if (!device || !out || device->sector_size != AHCI_SECTOR_SIZE ||
        !count || lba >= device->sector_count ||
        static_cast<uint64_t>(count) > device->sector_count - lba)
        return false;

    auto* port = static_cast<AhciPortContext*>(device->context);
    auto* output = static_cast<uint8_t*>(out);
    while (count)
    {
        const uint32_t chunk =
            count > AHCI_MAX_SECTORS_PER_COMMAND
                ? AHCI_MAX_SECTORS_PER_COMMAND : count;
        if (!ahci_issue(*port, ATA_CMD_READ_DMA_EXT, lba, chunk, false, port->dma))
            return false;
        copy_bytes(output, port->dma,
                   static_cast<uint64_t>(chunk) * AHCI_SECTOR_SIZE);
        output += static_cast<uint64_t>(chunk) * AHCI_SECTOR_SIZE;
        lba += chunk;
        count -= chunk;
    }
    return true;
}

static bool ahci_write(const BlockDevice* device, uint64_t lba,
                       uint32_t count, const void* input)
{
    if (!device || !input || device->sector_size != AHCI_SECTOR_SIZE ||
        !count || lba >= device->sector_count ||
        static_cast<uint64_t>(count) > device->sector_count - lba)
        return false;

    auto* port = static_cast<AhciPortContext*>(device->context);
    const auto* input_bytes = static_cast<const uint8_t*>(input);
    while (count)
    {
        const uint32_t chunk =
            count > AHCI_MAX_SECTORS_PER_COMMAND
                ? AHCI_MAX_SECTORS_PER_COMMAND : count;
        if (!ahci_issue(*port, ATA_CMD_WRITE_DMA_EXT, lba, chunk, true,
                        const_cast<uint8_t*>(input_bytes)))
            return false;
        input_bytes += static_cast<uint64_t>(chunk) * AHCI_SECTOR_SIZE;
        lba += chunk;
        count -= chunk;
    }
    return true;
}

static bool ahci_flush(const BlockDevice* device)
{
    if (!device || !device->context)
        return false;

    auto* port = static_cast<AhciPortContext*>(device->context);
    if (!port->initialized)
        return false;

    const int slot = ahci_find_slot(*port, 32);
    if (slot < 0)
        return false;

    if (!wait_clear(ahci_reg(port->regs, AHCI_PxTFD), AHCI_TFD_BSY | AHCI_TFD_DRQ))
        return false;

    *ahci_reg(port->regs, AHCI_PxIS) = 0xFFFFFFFFU;

    auto* header = reinterpret_cast<AhciCmdHeader*>(
        port->command_list + static_cast<uint64_t>(slot) * sizeof(AhciCmdHeader));
    zero_bytes(header, sizeof(AhciCmdHeader));
    header->flags = 5U;
    header->prdt_length = 0;
    header->command_table =
        static_cast<uint32_t>(port->table_physical & 0xFFFFFFFFULL);
    header->command_table_upper =
        static_cast<uint32_t>(port->table_physical >> 32);

    zero_bytes(port->table, 0x1000);
    auto* fis = reinterpret_cast<AhciFisH2D*>(port->table);
    fis->type = ATA_FIS_REG_H2D;
    fis->flags = 0x80;
    fis->command = 0xEA;
    fis->device = 0x40;

    asm volatile("" ::: "memory");
    *ahci_reg(port->regs, AHCI_PxCI) = 1U << slot;

    for (uint32_t spin = 0; spin < AHCI_TIMEOUT; ++spin)
    {
        if ((*ahci_reg(port->regs, AHCI_PxIS) & AHCI_IS_TFES) != 0)
            return false;
        if ((*ahci_reg(port->regs, AHCI_PxCI) & (1U << slot)) == 0)
            break;
        if (spin + 1 == AHCI_TIMEOUT)
            return false;
    }

    if ((*ahci_reg(port->regs, AHCI_PxIS) & AHCI_IS_TFES) != 0)
        return false;

    *ahci_reg(port->regs, AHCI_PxIS) = 0xFFFFFFFFU;
    return true;
}

static bool ahci_prepare_port(uint64_t hba_base, uint32_t port_number,
                              uint32_t slot_count)
{
    auto& port = ahci_ports[port_number];
    port.regs = reinterpret_cast<volatile uint8_t*>(
        paging_physical_to_virtual(hba_base + AHCI_PORT_BASE +
                                   static_cast<uint64_t>(port_number) * AHCI_PORT_STRIDE));
    if (!port.regs)
        return false;

    if (!ahci_stop(port))
        return false;

    port.command_list_physical = pmm_alloc_page_above(0x00200000ULL);
    port.fis_physical = pmm_alloc_page_above(0x00200000ULL);
    port.table_physical = pmm_alloc_page_above(0x00200000ULL);
    port.dma_physical = pmm_alloc_contiguous(2);
    if (!port.command_list_physical ||
        !port.fis_physical ||
        !port.table_physical ||
        !port.dma_physical)
    {
        ahci_release_port(port);
        return false;
    }

    port.command_list = reinterpret_cast<uint8_t*>(
        paging_physical_to_virtual(port.command_list_physical));
    port.fis = reinterpret_cast<uint8_t*>(
        paging_physical_to_virtual(port.fis_physical));
    port.table = reinterpret_cast<uint8_t*>(
        paging_physical_to_virtual(port.table_physical));
    port.dma = reinterpret_cast<uint8_t*>(
        paging_physical_to_virtual(port.dma_physical));
    if (!port.command_list || !port.fis || !port.table || !port.dma)
    {
        ahci_release_port(port);
        return false;
    }

    zero_bytes(port.command_list, 1024);
    zero_bytes(port.fis, 256);
    zero_bytes(port.table, 4096);
    zero_bytes(port.dma, 8192);

    *ahci_reg(port.regs, AHCI_PxCLB) =
        static_cast<uint32_t>(port.command_list_physical);
    *ahci_reg(port.regs, AHCI_PxCLBU) =
        static_cast<uint32_t>(port.command_list_physical >> 32);
    *ahci_reg(port.regs, AHCI_PxFB) =
        static_cast<uint32_t>(port.fis_physical);
    *ahci_reg(port.regs, AHCI_PxFBU) =
        static_cast<uint32_t>(port.fis_physical >> 32);

    const uint32_t entries = slot_count > 32 ? 32 : slot_count;
    for (uint32_t slot = 0; slot < entries; ++slot)
    {
        auto* header = reinterpret_cast<AhciCmdHeader*>(
            port.command_list + static_cast<uint64_t>(slot) * sizeof(AhciCmdHeader));
        header->command_table =
            static_cast<uint32_t>(port.table_physical);
        header->command_table_upper =
            static_cast<uint32_t>(port.table_physical >> 32);
    }

    *ahci_reg(port.regs, AHCI_PxIS) = 0xFFFFFFFFU;
    *ahci_reg(port.regs, AHCI_PxIE) = 0;
    if (!ahci_start(port))
    {
        ahci_release_port(port);
        return false;
    }

    port.initialized = true;
    return true;
}

static bool ahci_attach_controller(const StorageController& controller)
{
    if (!controller.pci || controller.type != STORAGE_CONTROLLER_AHCI)
        return false;
    if (!pci_enable_bus_master(controller.pci))
        return false;

    const uint64_t virtual_base =
        paging_physical_to_virtual(controller.mmio_base);
    if (!virtual_base)
        return false;

    storage_debug("AHCI ATTACH MMIO OK\n");
    auto* hba = reinterpret_cast<volatile uint8_t*>(virtual_base);
    auto* ghc = ahci_reg(hba, AHCI_GHC);
    *ghc |= (1U << 31);

    storage_debug("AHCI ATTACH HBA OK\n");
    const uint32_t cap = *ahci_reg(hba, AHCI_CAP);
    const uint32_t slots = ((cap >> 8) & 0x1FU) + 1U;
    const uint32_t ports = (cap & 0x1FU) + 1U;
    const uint32_t pi = *ahci_reg(hba, AHCI_PI);

    storage_debug("AHCI ATTACH CAP OK\n");
    for (uint32_t port_number = 0;
         port_number < ports && port_number < 32; ++port_number)
    {
        if ((pi & (1U << port_number)) == 0)
            continue;

        volatile uint8_t* port_regs =
            reinterpret_cast<volatile uint8_t*>(
                virtual_base + AHCI_PORT_BASE +
                static_cast<uint64_t>(port_number) * AHCI_PORT_STRIDE);
        const uint32_t ssts = *ahci_reg(port_regs, AHCI_PxSSTS);
        if ((ssts & 0xFU) != 3U || ((ssts >> 8) & 0xFU) != 1U)
            continue;
        if (*ahci_reg(port_regs, AHCI_PxSIG) != AHCI_SIG_SATA)
            continue;

        storage_debug("AHCI PORT PREPARE\n");
        if (!ahci_prepare_port(controller.mmio_base, port_number, slots))
            continue;
        auto& port = ahci_ports[port_number];
        storage_debug("AHCI PORT READY\n");
        if (!ahci_identify(port))
        {
            ahci_release_port(port);
            continue;
        }

        BlockDevice device{
            0, BLOCK_DEVICE_AHCI, AHCI_SECTOR_SIZE, port.sector_count,
            ahci_read, ahci_write, ahci_flush, &port
        };
        uint32_t id = 0;
        if (block_register(&device, &id))
        {
            port.block_id = id;
            ++real_block_device_count;
        }
        else
        {
            ahci_release_port(port);
        }
    }

    return true;
}

static bool add_controller(StorageControllerType type,const PciDevice* pci,uint64_t mmio)
{
    if (!pci||controller_count>=STORAGE_MAX_CONTROLLERS)
        return false;
    controllers[controller_count++] = {type,mmio,pci};
    return true;
}

extern "C" bool storage_initialize()
{
    if (storage_initialized)
        return true;

    storage_debug("STORAGE INIT START\n");
    controller_count=0;
    real_block_device_count=0;
    for (unsigned int i=0;i<STORAGE_MAX_CONTROLLERS;++i)
        controllers[i]={};
    for (unsigned int i=0;i<32;++i)
        ahci_ports[i]={};

    if (!pci_device_count() && !pci_initialize())
        return false;

    storage_debug("STORAGE INIT PCI OK\n");

    for (unsigned int i=0;i<pci_device_count();++i)
    {
        const PciDevice* device=&pci_devices()[i];
        if (device->class_code!=0x01)
            continue;

        if (device->subclass==0x06 && device->programming_interface==0x01)
        {
            const PciBar* bar=pci_get_bar(device,5);
            if (bar&&bar->present&&bar->base)
                add_controller(STORAGE_CONTROLLER_AHCI,device,bar->base);
        }
        else if (device->subclass==0x08 && device->programming_interface==0x02)
        {
            const PciBar* bar=pci_get_bar(device,0);
            if (bar&&bar->present&&bar->base)
                add_controller(STORAGE_CONTROLLER_NVME,device,bar->base);
        }
    }

    storage_debug("STORAGE INIT CONTROLLERS OK\n");
    for (unsigned int i=0;i<controller_count;++i)
        if (controllers[i].type==STORAGE_CONTROLLER_AHCI)
        {
            storage_debug("STORAGE INIT AHCI ATTACH\n");
            if (!ahci_attach_controller(controllers[i]))
                storage_debug("STORAGE INIT AHCI ATTACH FAIL\n");
            else
                storage_debug("STORAGE INIT AHCI ATTACH OK\n");
        }

    storage_initialized = true;
    return true;
}

extern "C" unsigned int storage_controller_count()
{
    return controller_count;
}

extern "C" const StorageController* storage_controller_get(unsigned int index)
{
    if (index>=controller_count)
        return nullptr;
    return &controllers[index];
}

extern "C" unsigned int storage_real_block_device_count()
{
    return real_block_device_count;
}

extern "C" bool storage_test()
{
    if (!storage_initialize())
        return false;

    const unsigned int saved_controller_count = controller_count;
    const unsigned int saved_real_block_device_count =
        real_block_device_count;
    if (!storage_initialize() ||
        controller_count != saved_controller_count ||
        real_block_device_count != saved_real_block_device_count)
        return false;

    for (unsigned int i=0;i<controller_count;++i)
    {
        const StorageController* c=&controllers[i];
        if (!c->pci||!c->mmio_base)
            return false;
        if (c->type!=STORAGE_CONTROLLER_AHCI &&
            c->type!=STORAGE_CONTROLLER_NVME)
            return false;
    }

    if (real_block_device_count != 0)
    {
        bool verified = false;
        for (unsigned int port_number = 0; port_number < 32; ++port_number)
        {
            const AhciPortContext& port = ahci_ports[port_number];
            if (!port.block_id)
                continue;

            const BlockDevice* device = block_get(port.block_id);
            if (!device || device->sector_count == 0)
                return false;

            uint8_t sector[AHCI_SECTOR_SIZE] = {};
            if (!block_read(device, 0, 1, sector) ||
                !block_flush(device))
                return false;

            storage_debug("STORAGE: GPT TRY\n");
            const bool registered = gpt_register_partitions(device);
            storage_debug(registered ? "STORAGE: GPT OK DEVICE\n" :
                                      "STORAGE: GPT NO DEVICE\n");
            verified = true;
            if (registered)
                break;
        }
        if (!verified)
            return false;
    }

    return true;
}
