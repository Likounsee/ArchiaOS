# ArchiaOS

ArchiaOS est un système d'exploitation x86_64 from scratch, avec son propre bootloader UEFI et son propre kernel freestanding.

Le projet ne repose ni sur Linux, ni sur GRUB, ni sur un autre OS sous-jacent.

> État actuel : bootstrap UEFI + kernel fonctionnels et validés par build et tests runtime sous QEMU/OVMF.
> Branche principale : ArchiaOS

## Objectif

Construire progressivement un OS x86_64 autonome :

    UEFI
      ↓
    ArchiaOS UEFI bootloader
      ↓
    Filesystem / ELF64 loader
      ↓
    BootInfo
      ↓
    Final UEFI memory map
      ↓
    ExitBootServices
      ↓
    x86_64 handoff
      ↓
    Kernel
      ↓
    CPU / interrupts / memory
      ↓
    Drivers / scheduler / processes
      ↓
    Userspace / syscalls / shell / GUI

Une fonctionnalité critique n'est considérée comme validée qu'après compilation et, lorsque possible, validation réelle dans QEMU.

## Architecture technique

| Élément | Choix |
|---|---|
| CPU | x86_64 |
| Firmware | UEFI |
| Bootloader | ArchiaOS UEFI loader |
| Kernel | freestanding C++20 + assembly |
| UEFI ABI | x86_64 UEFI calling convention |
| Kernel ABI | SysV AMD64 |
| Compiler | LLVM / Clang |
| Linker | LLD |
| Build | CMake |
| Runtime validation | QEMU + OVMF |
| Hardware tables | ACPI / SMBIOS |
| Graphics bootstrap | UEFI GOP |

## Boot chain

Le bootloader effectue actuellement :

1. entrée UEFI ;
2. découverte du filesystem ;
3. chargement de EFI\ARCHIAOS\ARCHIAOS_kernel.elf ;
4. validation ELF64/x86_64 et des segments PT_LOAD ;
5. validation de l'entry point ;
6. chargement des segments avec rollback en cas d'erreur ;
7. découverte GOP ;
8. découverte ACPI / SMBIOS ;
9. construction du BootInfo ;
10. allocation des métadonnées PMM ;
11. récupération de la memory map finale ;
12. ExitBootServices ;
13. handoff assembly ;
14. entrée dans kernel_entry puis kernel_main.

Le chemin de memory map peut agrandir son buffer avant ExitBootServices et le bootloader ne réutilise plus les services UEFI après une sortie réussie.

## BootInfo

Bootloader et kernel partagent une structure BootInfo version 3.

Elle contient notamment :
- magic, version et taille ;
- memory map et taille des descripteurs ;
- framebuffer ;
- ACPI RSDP ;
- SMBIOS ;
- adresse/taille du kernel ;
- adresse/taille du BootInfo ;
- métadonnées PMM ;
- informations de version du bootloader.

Le kernel valide les tailles, les bornes et la cohérence de la memory map avant utilisation.

## Organisation du dépôt

    .
    ├── .github/
    │   └── workflows/
    │       └── archiaos-build.yml
    ├── ArchiaOS/
    │   ├── CMakeLists.txt
    │   ├── boot/
    │   │   └── uefi/
    │   │       ├── include/          # API et structures du bootloader
    │   │       └── x86_64/           # implémentation UEFI x86_64
    │   ├── common/
    │   │   └── boot_info.h           # ABI partagé bootloader ↔ kernel
    │   ├── kernel/
    │   │   └── x86_64/
    │   │       ├── core/             # entrée et orchestration du kernel
    │   │       ├── cpu/              # CPU, GDT/TSS/IDT, exceptions, ACPI, LAPIC
    │   │       └── memory/           # PMM, paging et primitives mémoire
    │   │           └── tests/        # tests runtime mémoire
    │   ├── docs/
    │   │   ├── architecture/        # architecture système
    │   │   └── hardware/             # validation matérielle
    │   └── tools/
    │       └── firmware/             # OVMF utilisé pour les tests
    ├── .gitignore
    └── README.md

Règles :
- boot/ contient uniquement le bootloader ;
- common/ contient uniquement l'ABI partagé ;
- kernel/x86_64/core/ contient le cœur du kernel et le linker script ;
- kernel/x86_64/cpu/ contient les mécanismes dépendants du CPU et des interruptions ;
- kernel/x86_64/memory/ contient PMM, paging et primitives mémoire ;
- kernel/x86_64/memory/tests/ contient les tests mémoire ;
- docs/ contient la documentation d'architecture ;
- les artefacts restent dans ArchiaOS/build/ et ne sont pas versionnés.

## Kernel

Initialisation actuelle :

    kernel_entry
        ↓
    kernel_main
        ↓
    BootInfo validation
        ↓
    GDT / TSS
        ↓
    IDT
        ↓
    PMM
        ↓
    Paging
        ↓
    CPU security/features
        ↓
    LAPIC / timer
        ↓
    ACPI / MADT

Le kernel détecte notamment le vendor, le modèle CPU, SSE4.1, AVX/OSXSAVE/XGETBV, NX, SMEP, SMAP, UMIP, x2APIC, la topology et la largeur d'adresses physiques.

## Gestion mémoire

### PMM

Le Physical Memory Manager :
- démarre les frames en état réservé ;
- libère uniquement les régions UEFI autorisées ;
- réserve kernel, BootInfo, memory map, framebuffer et métadonnées PMM ;
- protège les frames réservées contre pmm_free_page() ;
- évite les doubles libérations ;
- vérifie les débordements et l'alignement ;
- supporte l'allocation contiguë ;
- possède des tests runtime.

La couverture physique bootstrap actuelle est limitée à 512 GiB. Ce plafond est explicite et ne signifie pas un support de toute la plage MAXPHYADDR possible.

### Paging

Le paging actuel comprend :
- PML4 / PDPT / PD ;
- mappings 1 GiB, 2 MiB et 4 KiB lorsque supportés ;
- split à la demande d'une feuille 1 GiB vers des tables plus fines ;
- mappings 2 MiB et 4 KiB ;
- Identity mapping bootstrap ;
- hiérarchie HHDM séparée ;
- isolation User/Supervisor ;
- RO/RW ;
- NX ;
- activation réelle de CR3 ;
- translation virtuelle/physique ;
- invalidation des alias nécessaires ;
- rollback des allocations de page tables.

HHDM actuel : 0xFFFF800000000000.

Le système de mémoire virtuelle complet, les address spaces par processus et le kernel high-half définitif restent à construire.

## CPU, exceptions et interruptions

### GDT / TSS / IDT
- GDT initialisée ;
- TSS chargée ;
- IST1 configurée pour #DF ;
- IST2 dédiée au NMI et IST3 au #MC ;
- 256 vecteurs IDT ;
- exceptions avec error code matériel distinguées ;
- récupération #UD et #PF uniquement pour les tests explicitement armés ;
- cld dans l'entrée ISR.

### LAPIC

Le Local APIC et son timer fonctionnent actuellement sous QEMU.

Le timer utilise une calibration basée sur l'Invariant TSC lorsqu'une fréquence TSC architecturale est disponible (CPUID 0x15, avec repli CPUID 0x16), puis conserve un fallback sûr lorsque la fréquence n'est pas énumérée.

Restent notamment à compléter : routage IOAPIC, x2APIC complet, SMP/AP startup, structures per-CPU et scheduler.

## ACPI

Le parseur couvre RSDP, RSDT, XSDT, MADT, Local APIC, x2APIC entries, IOAPIC et Interrupt Source Overrides.

Les accès aux structures ACPI sont contrôlés par des vérifications de bornes et d'overflow.

## ELF loader

Le loader vérifie notamment ELF64/x86_64, les bornes des headers, les PT_LOAD, l'alignement, l'entry point, l'absence de W+X, l'entry point file-backed, le rollback des allocations et le zeroing des pages allouées.

Les segments ELF sont désormais alloués comme mémoire UEFI LoaderData : le type mémoire UEFI décrit la propriété/lifetime du chargement, tandis que les permissions finales d'exécution, d'écriture et de lecture sont établies par le paging du kernel.

## Framebuffer

Le bootloader sait gérer l'absence de GOP.

Limitation actuelle : le kernel exige encore un framebuffer valide pendant son bootstrap. Le bootloader et le kernel ne sont donc pas encore totalement headless end-to-end.

## Tests et validation

La validation principale utilise QEMU x86_64, q35, OVMF et le debug console 0xE9.

Les tests runtime couvrent notamment BootInfo, memory map, GDT, TSS, IDT, PMM, allocation contiguë, paging, HHDM translation, accès mémoire HHDM, RO/NX, page faults, invalid opcode, LAPIC/timer et ACPI/MADT.

La CI teste également plusieurs modèles CPU QEMU.

## Documentation

- docs/architecture/cpu-compatibility.md
- docs/architecture/cpu-security.md
- docs/architecture/memory-management.md
- docs/architecture/virtual-address-space.md
- docs/hardware/cpu-validation.md

La documentation doit décrire l'architecture actuelle, pas une architecture future supposée.

## Compilation sous Windows

    cd C:\Users\Likounsee\Documents\ArchiaOS
    git checkout ArchiaOS
    git pull origin ArchiaOS
    cd .\ArchiaOS
    cmake -S . -B build
    cmake --build build --config Release

Artefacts attendus :

    build/ARCHIAOS_kernel.elf
    build/BOOTX64.EFI
    build/esp/EFI/BOOT/BOOTX64.EFI
    build/esp/EFI/ARCHIAOS/ARCHIAOS_kernel.elf

Nettoyage :

    Remove-Item -Recurse -Force .\build
    cmake -S . -B build
    cmake --build build --config Release

Ne pas supprimer ni modifier C:\Users\Likounsee\Documents\novos.vhdx. Ce fichier n'est pas nécessaire au nettoyage ou à la compilation du projet.

## Test QEMU sous Windows

Le workflow local utilise QEMU + OVMF et le debug console 0xE9. Le firmware de test se trouve dans ArchiaOS/tools/firmware/x86_64/ovmf/.

Exemple de lancement :

    cd C:\Users\Likounsee\Documents\ArchiaOS
    Copy-Item .\ArchiaOS\tools\firmware\x86_64\ovmf\OVMF_VARS.4m.fd .\ArchiaOS\build\OVMF_VARS.4m.fd -Force
    & "C:\Program Files\qemu\qemu-system-x86_64.exe" -machine q35 -m 256M -drive "if=pflash,format=raw,readonly=on,file=ArchiaOS\tools\firmware\x86_64\ovmf\OVMF_CODE.4m.fd" -drive "if=pflash,format=raw,file=ArchiaOS\build\OVMF_VARS.4m.fd" -drive "format=raw,file=fat:rw:ArchiaOS\build\esp" -display none -serial none -monitor none -debugcon stdio -global isa-debugcon.iobase=0xe9 -no-reboot -no-shutdown

## CI

Le workflow est .github/workflows/archiaos-build.yml.

Il construit le bootloader et le kernel, vérifie les artefacts ELF/EFI, démarre QEMU/OVMF, vérifie les marqueurs runtime et exécute la matrice de compatibilité CPU.

## État de l'audit

L'audit initial a identifié 43 findings.

Les corrections intégrées couvrent notamment #UD, #DF/IST1, protection des frames réservées, rollback paging, séparation Identity/HHDM, validation ACPI, durcissement ELF, validation BootInfo, détection CPU/physical address width, flags de compilation renforcés et memory-map dynamique avant ExitBootServices.

Des points restent ouverts : allocation physique du kernel encore contrainte par son modèle d'adressage actuel, support complet de la plage MAXPHYADDR au-delà du plafond bootstrap 512 GiB, politique PAT/cache complète dans tous les futurs chemins VM, frame d'exception inter-privilege plus complète, routage IOAPIC, x2APIC complet, linker/relocation pour une vraie allocation kernel indépendante, migration high-half définitive, SMP, scheduler, userspace et address spaces.

## Prochaines étapes

### Mémoire
- [x] PMM
- [x] allocation de pages
- [x] allocation contiguë
- [x] bootstrap paging
- [x] CR3
- [x] HHDM bootstrap
- [x] pages 1 GiB bootstrap + split à la demande
- [x] flags cache PWT/PCD sur les mappings 4 KiB
- [ ] virtual address manager
- [ ] kernel heap
- [ ] address spaces
- [ ] page fault production
- [ ] high-half kernel définitif

### CPU / interruptions
- [x] GDT
- [x] TSS
- [x] IDT
- [x] exceptions de base
- [x] LAPIC
- [x] timer bootstrap calibré avec fallback
- [x] IST dédiées NMI / #MC
- [ ] IOAPIC routing
- [ ] x2APIC complet
- [ ] SMP
- [ ] per-CPU
- [ ] scheduler

### Kernel
- [ ] heap
- [ ] threads
- [ ] context switching
- [ ] processus
- [ ] ring 3
- [ ] syscalls
- [ ] IPC

### Drivers / stockage
- [ ] PCI / PCIe
- [ ] NVMe / AHCI
- [ ] USB
- [ ] clavier
- [ ] souris
- [ ] réseau
- [ ] audio
- [ ] VFS
- [ ] filesystem
- [ ] stockage persistant

### Userspace
- [ ] init
- [ ] shell
- [ ] libc minimale
- [ ] gestionnaire de processus
- [ ] GUI
- [ ] applications natives

## Principes de maintenance

1. Garder les responsabilités séparées par dossier.
2. Garder les tests proches du sous-système qu'ils valident, mais séparés du code de production.
3. Ne pas mettre de fichiers générés dans Git.
4. Modifier l'ABI BootInfo avec une nouvelle version et des validations de taille.
5. Documenter les choix d'architecture dans docs/.
6. Ne considérer un changement critique comme terminé qu'après build + validation runtime.
7. Préférer les changements ciblés aux refactors massifs lorsque le comportement n'a pas besoin de changer.

## Licence

Aucune licence open source n'a encore été choisie pour ArchiaOS.