# NovOs

**NovOs** is a completely independent operating system built from scratch.

It is not based on Linux, Windows, or another existing operating system. The project is intended to develop its own kernel, boot process, memory management, interrupt infrastructure, drivers, filesystem, system services, security model, compatibility layers, and user experience.

> **Status:** Early development / experimental  
> **Current milestone:** x86-64 boot and kernel foundation validated in QEMU.

---

## Vision

NovOs aims to be a practical, lightweight, secure, configurable operating system with its own architecture and identity.

Core principles:

- Built from scratch
- No Linux kernel
- No Windows codebase
- Native low-level control
- Lightweight system design
- Strong security by default
- Local-first operation
- User-controlled configuration
- No unnecessary software
- No forced telemetry
- No mandatory online account
- No advertising inside the operating system
- No mandatory web browser
- Centralized system settings and updates
- Recoverable and maintainable system architecture

---

## Architecture

| Architecture | Planned support |
|---|---|
| x86-64 | Native |
| ARM64 | Native |
| x86-32 | Application compatibility on x86-64 |
| ARM32 | Application compatibility layer on ARM64 |

Architecture-specific code is kept separate from shared operating-system components.

---

## Boot

The long-term boot path is:

```text
UEFI / BIOS / CSM
        |
        v
NovOs Bootloader
        |
        v
ELF64 Kernel
        |
        v
NovOs Kernel
```

UEFI is the primary target during early development. BIOS / Legacy / CSM support is planned for a later stage.

### Validated boot foundation

The current x86-64 path has been tested in QEMU with OVMF:

```text
UEFI
  ↓
BOOTX64.EFI
  ↓
ELF loader
  ↓
ExitBootServices
  ↓
BootInfo
  ↓
kernel_entry
  ↓
kernel_main
```

The bootloader successfully loads the kernel ELF, obtains the final UEFI memory map, exits boot services, and transfers control to the kernel.

---

## Kernel foundation

The current kernel foundation includes:

- BootInfo validation
- UEFI memory-map handoff
- Physical memory manager (PMM)
- Page-table creation and identity mapping
- GDT initialization
- TSS initialization
- IDT initialization
- CPU exception entry stubs
- #UD exception handling and return
- Double Fault handling through IST1
- External IRQ vector infrastructure (32–47)
- Local APIC initialization and software enable
- Local APIC periodic timer IRQ
- Interrupt acknowledgement through Local APIC EOI
- Legacy 8259 PIC masking during APIC mode initialization
- ACPI RSDP / XSDT / RSDT discovery
- ACPI MADT discovery and basic processor / I/O APIC enumeration

These components have been exercised through QEMU tests.

The current test sequence reaches the following validated end state:

```text
PMM TEST PASS
PAGING TEST PASS
GDT INITIALIZED
TSS INITIALIZED
IDT INITIALIZED
#UD RETURNED SUCCESSFULLY
DOUBLE FAULT HANDLER RUNNING
IST1 STACK VALIDATED
```

The Double Fault test intentionally stops in the fatal handler after validating the IST1 stack.

---

## Development languages

NovOs primarily uses:

- **C++** for most operating-system components
- **Assembly** for architecture-specific low-level operations

Assembly is kept focused on areas requiring direct CPU control, such as boot entry, interrupt entry/exit, context switching, and architecture-specific primitives.

---

## Memory and resource goals

The long-term design target is approximately:

**≤ 3 GB RAM for the kernel, essential services, and core system applications under normal operation.**

This is a design target, not a claim about the current development build.

The project prioritizes explicit resource ownership, small core components, limited background services, and configurable features.

---

## User experience

NovOs is planned as a single operating-system edition. Profiles and settings will control behavior instead of splitting the platform into multiple editions.

Planned centralized management includes:

- System settings
- Hardware configuration
- Drivers
- Updates
- Security
- Recovery
- Storage
- User accounts
- Applications
- Compatibility features

---

## Privacy

NovOs is designed as a local-first operating system.

A local account should be sufficient for normal use. The platform is not intended to require:

- A mandatory online account
- Mandatory cloud synchronization
- Mandatory telemetry
- Advertising
- A mandatory browser

Future reporting features should remain explicitly controlled by the user.

---

## Security

Security is intended to be part of the architecture.

Planned areas include:

- Secure Boot
- Signed drivers
- Driver isolation
- Process isolation
- Application sandboxing
- Permission controls
- Secure system updates
- Firewall functionality
- Recovery mechanisms
- Protected system components
- System integrity mechanisms

---

## Drivers and hardware

NovOs will use its own driver framework.

The long-term hardware architecture is intended to cover:

- CPU
- Memory
- PCI / PCIe
- USB
- Storage
- Network
- Audio
- Graphics
- Input devices
- Power management

A native hardware and driver management center is planned. Driver support will be added progressively rather than assuming manufacturers provide NovOs-specific drivers.

---

## Filesystem

NovOs is planned to use its own native filesystem rather than adopting an existing desktop filesystem as its final native format.

Planned capabilities include:

- Files and directories
- Permissions
- Metadata
- Journaling / crash consistency
- Storage management
- Recovery support
- Snapshots
- System integrity

The filesystem design will be developed as the storage architecture matures.

---

## Recovery and updates

Native recovery is planned to include:

- Safe Mode
- Snapshots
- Rollback
- System repair
- Recoverable updates
- Failure isolation

System updates will be centrally managed with signed components, verification, and rollback where possible.

---

## Compatibility

### Windows

A future **WinCompat** environment is planned:

```text
Windows application
       ↓
PE / COFF loader
       ↓
WinCompat
       ↓
NovOs Windows API implementation
       ↓
NovOs kernel / services
```

The intended model is API compatibility, not CPU emulation.

### Linux

A Linux compatibility environment is planned for a later stage, using a subsystem and/or virtualized environment rather than incorporating the Linux kernel into NovOs.

---

## System applications

Planned native NovOs applications include:

- Settings
- File manager
- Hardware management
- Driver management
- Update management
- Security management
- Recovery
- Terminal
- System monitoring

---

## Error reporting

Future error reporting is intended to provide two explicit choices:

- **Do not send**
- **Send report**

No hidden or forced telemetry is planned.

---

## Roadmap

### Phase 1 — Boot foundation
- [x] Repository and project structure
- [x] x86-64 UEFI boot path
- [x] ELF64 kernel loading
- [x] Kernel entry point
- [x] ExitBootServices
- [x] BootInfo handoff
- [x] Firmware memory-map handoff
- [x] PMM foundation
- [x] Paging foundation
- [x] GDT
- [x] TSS
- [x] IDT
- [x] CPU exception entry
- [x] #UD return test
- [x] Double Fault / IST1 test

### Phase 2 — Kernel foundation
- [ ] Complete exception handling
- [x] IDT external IRQ vectors 32–47
- [x] Local APIC initialization
- [x] Local APIC timer IRQ infrastructure
- [x] Local APIC EOI handling
- [x] Legacy PIC masking
- [x] ACPI RSDP / XSDT / RSDT discovery
- [x] ACPI MADT discovery
- [x] Basic I/O APIC enumeration
- [ ] QEMU validation of Local APIC timer IRQ
- [ ] I/O APIC interrupt routing
- [ ] IRQ registration / handler framework
- [ ] Kernel allocator
- [ ] CPU initialization
- [ ] System calls
- [ ] User/kernel separation

### Phase 3 — Process and execution model
- [ ] Processes
- [ ] Threads
- [ ] Scheduler
- [ ] Context switching
- [ ] IPC
- [ ] Permissions

### Phase 4 — Hardware and drivers
- [ ] PCI / PCIe
- [ ] ACPI
- [ ] USB
- [ ] Storage controllers
- [ ] Network
- [ ] Input
- [ ] Graphics
- [ ] Audio
- [ ] Power management
- [ ] Native driver framework

### Phase 5 — Storage
- [ ] Native NovOs filesystem
- [ ] Storage manager
- [ ] File APIs
- [ ] Permissions
- [ ] Journaling / consistency
- [ ] Snapshots
- [ ] Recovery support

### Phase 6 — System services
- [ ] Service manager
- [ ] Device management
- [ ] Networking stack
- [ ] Security services
- [ ] Update system
- [ ] Logging
- [ ] Recovery environment

### Phase 7 — Graphics and desktop
- [ ] Graphics subsystem
- [ ] Window manager
- [ ] Input system
- [ ] Desktop
- [ ] Native system UI
- [ ] Settings
- [ ] File manager
- [ ] System management tools

### Phase 8 — Security hardening
- [ ] Secure Boot integration
- [ ] Signed drivers
- [ ] Sandboxing
- [ ] Permission system
- [ ] Secure updates
- [ ] Recovery / rollback
- [ ] System integrity

### Phase 9 — Compatibility
- [ ] x86-32 compatibility
- [ ] ARM32 compatibility layer
- [ ] WinCompat
- [ ] PE / COFF support
- [ ] Windows API compatibility
- [ ] Linux compatibility environment

### Phase 10 — Platform expansion
- [ ] Native ARM64 boot
- [ ] Native ARM64 kernel
- [ ] Additional hardware platforms
- [ ] BIOS / Legacy / CSM support
- [ ] Broader device support

---

## Repository structure

The repository is organized by responsibility and architecture:

```text
NovOs/
├── boot/
│   └── uefi/
│       ├── include/
│       └── x86_64/
├── common/
│   └── boot_info.h
├── kernel/
│   └── x86_64/
│       ├── core/
│       │   ├── entry.S
│       │   ├── kernel.cpp
│       │   └── kernel.ld
│       ├── cpu/
│       │   ├── gdt.*
│       │   ├── idt.*
│       │   ├── tss.*
│       │   └── exception.cpp
│       └── memory/
│           ├── pmm.*
│           ├── pmm_test.cpp
│           ├── paging.*
│           └── paging_test.cpp
└── tools/
    └── firmware/
        └── x86_64/
            └── ovmf/
```

Generated build outputs are not part of the source tree.

The source tree was recently reorganized by responsibility and the reorganized layout has been rebuilt and validated successfully in QEMU.

---

## Development environment

Current development uses:

- **C++20**
- **LLVM / Clang**
- **LLD**
- **CMake**
- **Ninja**
- **QEMU**
- **OVMF**

Development is currently focused on x86-64.

---

## Testing philosophy

Every major subsystem should be:

1. Designed
2. Implemented
3. Built
4. Tested
5. Validated in QEMU or on real hardware
6. Used as a foundation only after validation

The project currently uses QEMU + OVMF for early boot and kernel validation.

---

## Project status

NovOs is **not production-ready**.

It is an active early-stage operating-system project. The current milestone establishes a working UEFI → bootloader → kernel path and validates the first low-level kernel subsystems.

The next development stage will continue by validating the new Local APIC timer IRQ path, then adding ACPI/MADT and I/O APIC routing for device interrupts.

---

## License

License information will be added as the project matures.
