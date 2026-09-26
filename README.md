# NovOs

**NovOs** is a completely independent operating system built from scratch.

It is not based on Linux, Windows, or another existing operating system. The project aims to build its own kernel, boot process, system services, graphics stack, filesystem, driver architecture, compatibility layers, security model, and user experience.

> **Status:** Early development / experimental  
> **Current milestone:** Boot chain established; BootInfo is the next kernel/boot interface milestone.

---

## Vision

NovOs is designed around a simple idea:

**Build an operating system that is powerful, lightweight, understandable, configurable, and respectful of its users.**

The goal is not to reproduce another operating system feature-for-feature. NovOs is intended to have its own architecture and identity while remaining practical for everyday computing.

### Core principles

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
- Centralized system settings
- Centralized system updates
- Recoverable and maintainable system architecture

---

## Architecture

### Supported architectures

NovOs is planned to support:

| Architecture | Support |
|---|---|
| x86-64 | Native |
| ARM64 | Native |
| x86-32 | Compatibility on x86-64 |
| ARM32 | Application compatibility layer on ARM64 |

The architecture is designed so that platform-specific low-level code can remain isolated while higher-level operating system components can be shared.

---

## Boot process

The initial boot path is:

~~~text
Firmware
   │
   ├── UEFI
   │
   └── BIOS / Legacy / CSM (planned)
          │
          ▼
   NovOs Bootloader
          │
          ▼
   ELF Kernel
          │
          ▼
   NovOs Kernel
~~~

UEFI is the primary target during early development.

BIOS / Legacy / CSM support is planned for a later stage.

### Current boot milestone

The project has already demonstrated the following working chain:

~~~text
UEFI
  ↓
BOOTX64.EFI
  ↓
ELF loader
  ↓
NovOs kernel
  ↓
ExitBootServices
  ↓
kernel_entry
  ↓
kernel_main
  ↓
NOVOS KERNEL STARTED
~~~

The next boot/kernel interface milestone is **BootInfo**, which will allow the bootloader to pass structured firmware information to the kernel.

---

## Kernel

The NovOs kernel is being developed from scratch.

The kernel will eventually provide the fundamental services required by the rest of the operating system, including:

- CPU and architecture initialization
- Memory management
- Virtual memory
- Interrupt management
- Process and thread management
- Scheduling
- Inter-process communication
- Kernel object/resource management
- Hardware abstraction
- Security primitives
- System calls
- Driver management
- Power management
- Storage management

The kernel will remain deliberately separated from higher-level system services whenever possible.

---

## Development languages

NovOs primarily uses:

- **C++** for most operating system components
- **Assembly** for architecture-specific low-level operations

Assembly will be kept focused on areas where direct CPU control is required, such as:

- Boot entry
- CPU context handling
- Interrupt entry/exit
- Context switching
- Architecture-specific primitives

---

## Memory and resource goals

NovOs is intended to remain lightweight.

The long-term target is approximately:

**≤ 3 GB RAM for the kernel, essential services, and core system applications under normal operation.**

This is a design target rather than a claim about the current development build.

The project will prioritize:

- Small core components
- Explicit resource ownership
- Limited background services
- Avoiding unnecessary resident software
- Efficient system services
- Configurable features

---

## User experience

NovOs will use a single operating system edition rather than splitting the platform into multiple artificial editions.

Instead, users will be able to configure their system through profiles and settings.

### System configuration

The operating system is planned to provide centralized management for:

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

The objective is to keep system administration understandable without scattering configuration across unrelated tools.

---

## Accounts and privacy

NovOs is designed as a **local-first operating system**.

A local account should be sufficient for normal use.

The operating system will not require:

- A mandatory online account
- Mandatory cloud synchronization
- Mandatory telemetry
- Advertising
- A mandatory browser

Telemetry, where applicable in future components, should be explicitly controlled by the user.

---

## Security

Security is intended to be part of the architecture rather than an afterthought.

Planned security features include:

- Secure Boot support
- Signed drivers
- Driver isolation
- Process isolation
- Application sandboxing
- Permission controls
- Secure system updates
- Firewall functionality
- Recovery mechanisms
- Protected system components
- Secure boot and recovery paths

Security decisions will be implemented progressively as the kernel and system architecture mature.

---

## Driver architecture

NovOs will have its own driver framework.

The long-term goal is to provide a centralized hardware and driver management system capable of:

- Detecting hardware
- Loading appropriate drivers
- Managing driver versions
- Reporting hardware status
- Handling driver failures
- Controlling permissions
- Supporting signed drivers

The driver architecture will be designed around NovOs rather than attempting to directly reuse another operating system's driver model.

---

## Filesystem

NovOs is planned to use its own native filesystem.

The filesystem will be designed specifically for NovOs rather than simply adopting an existing desktop filesystem as the final native format.

Planned areas include:

- Files and directories
- Permissions
- Metadata
- Journaling / crash consistency
- Storage management
- Recovery support
- Snapshots
- System integrity

The final filesystem design will be developed as the storage architecture matures.

---

## Recovery and reliability

NovOs is planned to include native recovery mechanisms.

These include:

- System recovery
- Safe Mode
- Snapshots
- Rollback
- System repair
- Recoverable updates
- Failure isolation

The goal is to make system failures recoverable without requiring a complete reinstall whenever possible.

Dynamic system changes should also avoid unnecessary reinstallations.

---

## Updates

System updates will be centrally managed.

The update architecture is planned to support:

- Centralized update management
- Secure update verification
- Signed system components
- Recovery from failed updates
- Rollback where possible
- Minimal unnecessary disruption

Updates should not silently compromise system integrity or user control.

---

## Compatibility

Compatibility is a long-term goal, but it will not define the core architecture.

### Windows compatibility

NovOs plans to provide a **WinCompat** environment in the future.

The intended approach is:

~~~text
Windows application
       ↓
PE / COFF loader
       ↓
WinCompat
       ↓
NovOs implementation of required Windows APIs
       ↓
NovOs kernel/services
~~~

This is intended to be an API compatibility layer, **not CPU emulation**.

The objective is to execute compatible Windows applications while keeping the NovOs kernel independent.

### Linux compatibility

A Linux compatibility environment is also planned for a later stage.

The current long-term direction is a subsystem and/or virtualized environment rather than incorporating the Linux kernel into NovOs itself.

---

## System applications

NovOs will eventually provide native system applications for core tasks such as:

- System Settings
- File management
- Hardware management
- Driver management
- Update management
- Security management
- Recovery
- Terminal
- System monitoring

These applications are intended to be native NovOs software rather than mandatory third-party bundles.

---

## Error reporting

NovOs is intended to keep crash/error reporting simple and explicit.

When an error report can be sent, the user should have two clear choices:

- **Do not send**
- **Send report**

There should be no forced telemetry hidden behind unrelated settings.

---

## Hardware support

The long-term goal is broad hardware support across supported architectures.

Initial development focuses on establishing the architecture and boot chain before expanding hardware support.

Planned areas include:

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

Hardware support will be added progressively through the NovOs driver framework.

---

## Project roadmap

The roadmap is intentionally incremental.

### Phase 1 — Boot foundation

- [x] Repository and project structure
- [x] x86-64 bootloader foundation
- [x] UEFI boot path
- [x] ELF64 kernel loading
- [x] Kernel entry point
- [x] ExitBootServices
- [x] Kernel execution in QEMU
- [ ] BootInfo contract
- [ ] Reliable firmware memory-map handoff

### Phase 2 — Kernel foundation

- [ ] GDT
- [ ] IDT
- [ ] TSS
- [ ] Exception handling
- [ ] Interrupt handling
- [ ] Physical memory management
- [ ] Virtual memory
- [ ] Kernel allocator
- [ ] CPU initialization
- [ ] Basic system calls

### Phase 3 — Process and execution model

- [ ] Processes
- [ ] Threads
- [ ] Scheduler
- [ ] Context switching
- [ ] User/kernel separation
- [ ] IPC
- [ ] Permissions

### Phase 4 — Hardware and drivers

- [ ] PCI/PCIe
- [ ] ACPI
- [ ] USB
- [ ] Storage controllers
- [ ] Network devices
- [ ] Input devices
- [ ] Graphics hardware
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
- [ ] Window management
- [ ] Input system
- [ ] Desktop environment
- [ ] Native system UI
- [ ] Settings application
- [ ] File manager
- [ ] System management tools

### Phase 8 — Security hardening

- [ ] Secure Boot integration
- [ ] Signed drivers
- [ ] Sandboxing
- [ ] Permission system
- [ ] Secure updates
- [ ] Recovery/rollback
- [ ] System integrity mechanisms

### Phase 9 — Compatibility

- [ ] x86-32 application compatibility
- [ ] ARM32 compatibility layer
- [ ] WinCompat
- [ ] PE/COFF support
- [ ] Windows API compatibility
- [ ] Linux compatibility environment

### Phase 10 — Platform expansion

- [ ] Native ARM64 boot
- [ ] Native ARM64 kernel
- [ ] Additional hardware platforms
- [ ] BIOS / Legacy / CSM support
- [ ] Broader device support

---

## Development philosophy

NovOs is developed incrementally.

Each major subsystem should be:

1. Designed
2. Implemented
3. Built
4. Tested
5. Validated in an emulator or on real hardware
6. Only then used as a foundation for the next subsystem

The project prioritizes correctness and understanding of the underlying system over rapidly accumulating features.

---

## Current development environment

The project currently uses:

- **C++20**
- **LLVM / Clang**
- **LLD**
- **CMake**
- **Ninja**
- **QEMU**
- **OVMF** for UEFI testing

Development is currently focused on **x86-64**.

---

## Repository structure

The project is organized around independent operating system components.

~~~text
NovOs/
├── boot/
│   └── uefi/
├── common/
├── kernel/
│   └── x86_64/
├── tools/
│   └── firmware/
└── build/
~~~

The structure will evolve as additional architectures and subsystems are introduced.

---

## Testing

NovOs is currently tested primarily through QEMU and OVMF.

The project uses virtualized boot testing to validate early kernel and bootloader changes before moving functionality to physical hardware.

Early milestones focus on proving each stage of the boot chain rather than attempting to boot a complete desktop environment immediately.

---

## Project status

NovOs is **not production-ready**.

It is an active early-stage operating system project.

At the current stage, the most important achievement is establishing a real boot path from UEFI to independently executing NovOs kernel code.

The project will continue to grow from this minimal foundation toward a complete operating system.

---

## License

License information will be added as the project matures.
