# ArchiaOS

ArchiaOS est un système d'exploitation **x86_64 généraliste, construit from scratch**, avec son propre bootloader UEFI et son propre kernel freestanding.

Le but final n'est pas de faire un simple noyau de démonstration ni un environnement qui dépend d'un autre OS. **L'objectif est de construire, étape par étape, un véritable OS autonome utilisable comme système principal**, dans l'esprit d'un OS moderne généraliste comme Windows : démarrage natif, gestion mémoire et CPU, processus et threads, pilotes matériels, stockage, réseau, sécurité, espace utilisateur, shell, environnement graphique et applications.

Cela ne signifie pas copier Windows ou réimplémenter son code. ArchiaOS doit fournir **ses propres interfaces, son propre kernel et sa propre architecture**, avec une expérience d'utilisation qui tende progressivement vers celle d'un OS de bureau complet.

> **État actuel :** le bootstrap bas niveau est fortement avancé et le kernel dispose maintenant d'un scheduler préemptif réellement per-CPU avec SMP/AP validé par CI sous QEMU/OVMF.
>
> **Branche de développement :** `ArchiaOS`

## Vision du projet

La trajectoire globale d'ArchiaOS est :

```
UEFI
  ↓
ArchiaOS UEFI bootloader
  ↓
ELF64 loader + BootInfo
  ↓
ExitBootServices
  ↓
Kernel x86_64
  ↓
CPU / GDT / TSS / IDT / exceptions
  ↓
PMM / paging / HHDM / virtual memory
  ↓
LAPIC / IOAPIC / IRQ / SMP
  ↓
per-CPU scheduler / threads / context switching
  ↓
processus / address spaces / ring 3 / syscalls
  ↓
drivers / PCIe / USB / clavier / souris / stockage / réseau / audio
  ↓
VFS / filesystem / init / libc
  ↓
shell + services système
  ↓
GUI / bureau / applications natives
  ↓
OS généraliste autonome
```

Une fonctionnalité critique n'est considérée comme validée qu'après compilation et, lorsque possible, **validation réelle dans QEMU**. La CI GitHub est la référence de validation automatisée.

## Objectif final : un OS complet

À long terme, ArchiaOS doit couvrir les grandes fonctions attendues d'un OS de bureau moderne :

- boot UEFI natif ;
- kernel 64-bit autonome ;
- mémoire virtuelle et isolation des espaces d'adresses ;
- processus et threads ;
- préemption et scheduling SMP ;
- mode utilisateur / ring 3 ;
- syscalls et IPC ;
- gestion des droits et primitives de sécurité ;
- pilotes PCI/PCIe et périphériques ;
- clavier, souris, USB et affichage ;
- stockage persistant et filesystem ;
- réseau et sockets ;
- audio ;
- gestion des services et du démarrage ;
- shell et outils système ;
- environnement graphique avec fenêtres, bureau et applications ;
- APIs natives stables pour les programmes utilisateurs ;
- installation, mise à jour et récupération du système.

**Windows sert ici de référence de portée fonctionnelle, pas de dépendance technique ni de cible de copie.** L'objectif est qu'ArchiaOS puisse un jour démarrer directement sur une machine et fournir l'ensemble de la pile nécessaire à un usage quotidien.

## État actuel

### Déjà fonctionnel et validé

- [x] Boot UEFI x86_64
- [x] Bootloader UEFI ArchiaOS
- [x] Chargement et validation ELF64
- [x] BootInfo partagé bootloader ↔ kernel
- [x] GOP / framebuffer bootstrap
- [x] ACPI / RSDP / RSDT / XSDT / MADT
- [x] SMBIOS
- [x] Memory map UEFI dynamique avant ExitBootServices
- [x] ExitBootServices + handoff assembly
- [x] PMM
- [x] Allocation physique contiguë
- [x] Paging x86_64
- [x] Identity mapping bootstrap
- [x] HHDM
- [x] Higher-half kernel bootstrap
- [x] Mapping 1 GiB / 2 MiB / 4 KiB selon support
- [x] Split à la demande des feuilles 1 GiB
- [x] Protection RO/RW/NX
- [x] Translation virtuelle ↔ physique
- [x] GDT / TSS / IDT
- [x] IST dédiées
- [x] Exceptions x86_64
- [x] LAPIC / xAPIC / x2APIC bootstrap
- [x] LAPIC timer
- [x] IOAPIC / IRQ
- [x] CPUID et détection des fonctionnalités CPU
- [x] Protections CPU : NX, CR0.WP, SMEP et autres capacités détectées/supportées
- [x] SMP / démarrage des AP
- [x] Structures et état per-CPU
- [x] Context switching réel
- [x] Scheduler round-robin
- [x] Scheduler préemptif
- [x] Scheduler réellement indépendant sur chaque CPU
- [x] Test runtime prouvant l'exécution et le context switching sur chaque CPU
- [x] Validation GitHub Actions sous QEMU/OVMF, y compris matrice de compatibilité CPU et smoke test SMP

### En construction

- [ ] Virtual address manager complet
- [ ] Kernel heap général
- [ ] Gestion complète des address spaces
- [ ] Page faults de production
- [ ] High-half kernel définitif et relocation/placement indépendant
- [ ] Threads kernel complets
- [ ] Processus
- [ ] Ring 3
- [ ] ABI de syscalls
- [ ] IPC
- [ ] Ordonnanceur de processus plus complet
- [ ] PCI / PCIe
- [ ] USB
- [ ] Clavier / souris
- [ ] NVMe / AHCI
- [ ] VFS / filesystem
- [ ] Réseau
- [ ] Audio
- [ ] init / services
- [ ] libc minimale
- [ ] shell utilisateur
- [ ] GUI / window manager
- [ ] applications natives
- [ ] installation et distribution

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

ArchiaOS est conçu pour rester indépendant de Linux, GRUB et de tout autre OS sous-jacent.

## Boot chain

Le bootloader effectue actuellement :

1. entrée UEFI ;
2. découverte du filesystem ;
3. chargement de `EFI\\ARCHIAOS\\ARCHIAOS_kernel.elf` ;
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
14. entrée dans `kernel_entry` puis `kernel_main`.

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

```
.
├── .github/
│   └── workflows/
│       └── archiaos-build.yml
├── ArchiaOS/
│   ├── CMakeLists.txt
│   ├── boot/
│   │   └── uefi/
│   │       ├── include/
│   │       └── x86_64/
│   ├── common/
│   │   └── boot_info.h
│   ├── kernel/
│   │   └── x86_64/
│   │       ├── core/
│   │       ├── cpu/
│   │       └── memory/
│   │           └── tests/
│   ├── docs/
│   │   ├── architecture/
│   │   └── hardware/
│   └── tools/
│       └── firmware/
├── .gitignore
└── README.md
```

Règles :

- `boot/` contient uniquement le bootloader ;
- `common/` contient uniquement l'ABI partagé ;
- `kernel/x86_64/core/` contient le cœur du kernel et le linker script ;
- `kernel/x86_64/cpu/` contient les mécanismes dépendants du CPU et des interruptions ;
- `kernel/x86_64/memory/` contient PMM, paging et primitives mémoire ;
- `kernel/x86_64/memory/tests/` contient les tests mémoire ;
- `docs/` contient la documentation d'architecture ;
- les artefacts restent dans `ArchiaOS/build/` et ne sont pas versionnés.

## Kernel

L'initialisation actuelle suit notamment :

```
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
Paging / HHDM
    ↓
CPU security/features
    ↓
LAPIC / timer
    ↓
ACPI / MADT / IOAPIC
    ↓
SMP / AP startup
    ↓
per-CPU scheduler
```

Le kernel détecte notamment le vendor, le modèle CPU, SSE4.1, AVX/OSXSAVE/XGETBV, NX, SMEP, SMAP, UMIP, x2APIC, la topology et la largeur d'adresses physiques.

## Gestion mémoire

### PMM

Le Physical Memory Manager :

- démarre les frames en état réservé ;
- libère uniquement les régions UEFI autorisées ;
- réserve kernel, BootInfo, memory map, framebuffer et métadonnées PMM ;
- protège les frames réservées contre `pmm_free_page()` ;
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
- Identity mapping bootstrap ;
- hiérarchie HHDM séparée ;
- isolation User/Supervisor ;
- RO/RW ;
- NX ;
- activation réelle de CR3 ;
- translation virtuelle/physique ;
- invalidation des alias nécessaires ;
- rollback des allocations de page tables.

HHDM actuel : `0xFFFF800000000000`.

La gestion complète de mémoire virtuelle, les address spaces indépendants par processus et le kernel high-half définitif restent à construire.

## CPU, exceptions et interruptions

### GDT / TSS / IDT

- GDT initialisée ;
- TSS chargée ;
- IST1 configurée pour #DF ;
- IST2 dédiée au NMI ;
- IST3 dédiée au #MC ;
- 256 vecteurs IDT ;
- exceptions avec error code matériel distinguées ;
- récupération #UD et #PF uniquement pour les tests explicitement armés ;
- `cld` dans l'entrée ISR.

### LAPIC / IOAPIC

Le Local APIC, son timer et le routage IRQ nécessaire au scheduler fonctionnent sous QEMU.

Le timer utilise une calibration basée sur l'Invariant TSC lorsqu'une fréquence TSC architecturale est disponible (CPUID 0x15, avec repli CPUID 0x16), puis conserve un fallback sûr lorsque la fréquence n'est pas énumérée.

Le parseur ACPI couvre notamment le MADT, Local APIC, x2APIC entries, IOAPIC et Interrupt Source Overrides.

Le support x2APIC existe au niveau nécessaire au bootstrap/détection, mais la couverture x2APIC complète reste à finaliser.

## SMP et scheduler

ArchiaOS démarre maintenant plusieurs CPU/AP et donne à chaque CPU son propre état de scheduler.

Le scheduler actuel comprend :

- état scheduler par CPU ;
- tâches et stacks indépendantes par CPU ;
- démarrage du scheduler sur les AP ;
- timer LAPIC local par CPU ;
- préemption par interruption timer ;
- context switching réel ;
- round-robin ;
- compteur de switches par CPU ;
- validation runtime que les tâches s'exécutent réellement sur chaque CPU.

Cette étape est importante pour la suite : les processus et threads utilisateurs devront pouvoir s'appuyer sur cette infrastructure plutôt que sur un scheduler mono-CPU de démonstration.

## ELF loader

Le loader vérifie notamment ELF64/x86_64, les bornes des headers, les PT_LOAD, l'alignement, l'entry point, l'absence de W+X, l'entry point file-backed, le rollback des allocations et le zeroing des pages allouées.

Les segments ELF sont alloués comme mémoire UEFI LoaderData : le type mémoire UEFI décrit la propriété/lifetime du chargement, tandis que les permissions finales d'exécution, d'écriture et de lecture sont établies par le paging du kernel.

## Framebuffer

Le bootloader sait gérer l'absence de GOP.

Limitation actuelle : le kernel exige encore un framebuffer valide pendant son bootstrap. Le bootloader et le kernel ne sont donc pas encore totalement headless end-to-end.

Le framebuffer actuel est une base de bootstrap ; le système graphique complet reste une étape future.

## Tests et validation

La validation principale utilise QEMU x86_64, q35, OVMF et le debug console 0xE9.

Les tests runtime couvrent notamment :

- BootInfo ;
- memory map ;
- GDT ;
- TSS ;
- IDT ;
- PMM ;
- allocation contiguë ;
- paging ;
- HHDM translation ;
- accès mémoire HHDM ;
- RO/NX ;
- page faults ;
- invalid opcode ;
- LAPIC/timer ;
- ACPI/MADT ;
- SMP ;
- scheduler préemptif et context switching per-CPU.

La CI teste également plusieurs modèles CPU QEMU et exécute un smoke test SMP.

## Documentation

La documentation d'architecture doit toujours décrire **l'état réellement implémenté**, pas une architecture future supposée.

Documents principaux :

- `docs/architecture/cpu-compatibility.md`
- `docs/architecture/cpu-security.md`
- `docs/architecture/memory-management.md`
- `docs/architecture/virtual-address-space.md`
- `docs/hardware/cpu-validation.md`

## Roadmap vers un OS de bureau complet

### Phase 1 — Fondation kernel

- [x] Boot UEFI
- [x] ELF64 loader
- [x] BootInfo
- [x] PMM
- [x] Paging / HHDM
- [x] GDT / TSS / IDT
- [x] Exceptions
- [x] LAPIC / IRQ
- [x] ACPI / IOAPIC
- [x] SMP / AP startup
- [x] per-CPU scheduler
- [x] préemption / context switching

### Phase 2 — Mémoire virtuelle et exécution

- [ ] virtual address manager
- [ ] kernel heap
- [ ] kernel threads complets
- [ ] address spaces
- [ ] page fault handler de production
- [ ] isolation utilisateur/kernel
- [ ] ring 3
- [ ] syscalls
- [ ] processus
- [ ] threads utilisateurs
- [ ] IPC / synchronisation avancée

### Phase 3 — Pilotes et matériel

- [ ] PCI / PCIe
- [ ] USB host controller
- [ ] clavier
- [ ] souris
- [ ] framebuffer/console robuste
- [ ] NVMe
- [ ] AHCI
- [ ] stockage générique
- [ ] réseau
- [ ] audio
- [ ] ACPI power management avancé

### Phase 4 — Services système

- [ ] VFS
- [ ] filesystem natif
- [ ] cache disque
- [ ] init
- [ ] gestionnaire de services
- [ ] gestionnaire de processus
- [ ] gestion des utilisateurs et permissions
- [ ] libc minimale
- [ ] shell utilisateur
- [ ] outils système

### Phase 5 — Bureau

- [ ] pilote graphique adapté au matériel
- [ ] compositor / window manager
- [ ] fenêtres
- [ ] pointeur et input complet
- [ ] bureau
- [ ] gestionnaire de fichiers
- [ ] terminal
- [ ] applications natives
- [ ] APIs graphiques et UI

### Phase 6 — OS utilisable au quotidien

- [ ] installateur
- [ ] partitionnement
- [ ] boot persistant
- [ ] mises à jour
- [ ] récupération système
- [ ] gestion logicielle
- [ ] réseau utilisateur complet
- [ ] sécurité système renforcée
- [ ] documentation utilisateur
- [ ] distribution stable x86_64

## Compilation sous Windows

```powershell
cd C:\Users\Likounsee\Documents\ArchiaOS
git checkout ArchiaOS
git pull origin ArchiaOS
cd .\ArchiaOS
cmake -S . -B build
cmake --build build --config Release
```

Artefacts attendus :

```
build/ARCHIAOS_kernel.elf
build/BOOTX64.EFI
build/esp/EFI/BOOT/BOOTX64.EFI
build/esp/EFI/ARCHIAOS/ARCHIAOS_kernel.elf
```

Nettoyage :

```powershell
Remove-Item -Recurse -Force .\build
cmake -S . -B build
cmake --build build --config Release
```

**Ne pas supprimer ni modifier `C:\Users\Likounsee\Documents\novos.vhdx`.** Ce fichier n'est pas nécessaire au nettoyage ou à la compilation du projet.

## Test QEMU sous Windows

Le workflow local utilise QEMU + OVMF et le debug console 0xE9. Le firmware de test se trouve dans `ArchiaOS/tools/firmware/x86_64/ovmf/`.

Exemple :

```powershell
cd C:\Users\Likounsee\Documents\ArchiaOS
Copy-Item .\ArchiaOS\tools\firmware\x86_64\ovmf\OVMF_VARS.4m.fd .\ArchiaOS\build\OVMF_VARS.4m.fd -Force
& "C:\Program Files\qemu\qemu-system-x86_64.exe" -machine q35 -m 256M -drive "if=pflash,format=raw,readonly=on,file=ArchiaOS\tools\firmware\x86_64\ovmf\OVMF_CODE.4m.fd" -drive "if=pflash,format=raw,file=ArchiaOS\build\OVMF_VARS.4m.fd" -drive "format=raw,file=fat:rw:ArchiaOS\build\esp" -display none -serial none -monitor none -debugcon stdio -global isa-debugcon.iobase=0xe9 -no-reboot -no-shutdown
```

## CI

Le workflow est `.github/workflows/archiaos-build.yml`.

Il :

1. construit le bootloader et le kernel ;
2. vérifie les artefacts ELF/EFI ;
3. démarre QEMU/OVMF ;
4. vérifie les marqueurs runtime ;
5. exécute la matrice de compatibilité CPU ;
6. exécute le smoke test SMP/scheduler.

Une fonctionnalité critique n'est pas considérée comme terminée tant que le build et les tests concernés ne sont pas verts.

## État de l'audit

L'audit initial avait identifié 43 findings.

De nombreuses corrections ont depuis été intégrées, notamment :

- #UD ;
- #DF / IST1 ;
- protection des frames réservées ;
- rollback paging ;
- séparation Identity/HHDM ;
- validation ACPI ;
- durcissement ELF ;
- validation BootInfo ;
- détection CPU / physical address width ;
- flags de compilation renforcés ;
- memory-map dynamique avant ExitBootServices ;
- SMP / AP startup ;
- scheduler préemptif et context switching per-CPU.

Les points encore ouverts concernent principalement la construction des couches nécessaires à un véritable OS utilisateur : gestion virtuelle complète, heap, address spaces, ring 3, syscalls, processus, pilotes, stockage, réseau, VFS, userspace et GUI.

## Principes de développement

1. Construire ArchiaOS comme un OS autonome, pas comme une application au-dessus d'un autre OS.
2. Garder les responsabilités séparées par sous-système.
3. Faire évoluer les ABI partagées avec des versions et des validations de taille.
4. Ne pas mettre de fichiers générés dans Git.
5. Documenter les décisions d'architecture dans `docs/`.
6. Tester les changements critiques dans QEMU.
7. Utiliser GitHub Actions comme validation reproductible.
8. Préférer les changements ciblés aux refactors massifs lorsque le comportement n'a pas besoin de changer.
9. Ne jamais présenter comme fonctionnelle une couche qui n'est pas réellement implémentée et testée.
10. Garder la roadmap orientée vers l'objectif final : **un OS de bureau généraliste complet et autonome**.

## Licence

Aucune licence open source n'a encore été choisie pour ArchiaOS.
