# ArchiaOS

**ArchiaOS** est un système d'exploitation x86_64 développé from scratch, avec son propre bootloader UEFI et son propre noyau.

Le projet ne repose pas sur Linux, GRUB ou un autre OS sous-jacent.

> **État :** fondations du bootloader et du kernel fonctionnelles et testées sous QEMU/OVMF.
> **Branche de développement :** ArchiaOS

---

## Sommaire

- Objectif
- Architecture
- Chaîne de démarrage
- Bootloader
- BootInfo
- Kernel
- Gestion mémoire
- Interruptions et CPU
- ACPI
- Framebuffer
- Tests et validation
- Arborescence
- Compilation sous Windows
- Test QEMU sous Windows
- CI GitHub
- Avancement
- Prochaines étapes

---

## Objectif

ArchiaOS vise à devenir un véritable OS x86_64 autonome, en construisant progressivement toutes les couches nécessaires :

    UEFI
      ↓
    Bootloader ArchiaOS
      ↓
    Filesystem / chargement ELF64
      ↓
    BootInfo
      ↓
    GetMemoryMap final
      ↓
    ExitBootServices
      ↓
    Handoff x86_64
      ↓
    Kernel
      ↓
    Mémoire / interruptions / CPU
      ↓
    Drivers / scheduler / processus
      ↓
    Userspace / syscalls / shell / GUI

Le projet suit une règle simple : un sous-système n'est considéré comme stable qu'après compilation et validation réelle dans QEMU lorsque cela est possible.

---

## Architecture

| Élément | Technologie |
|---|---|
| Architecture | x86_64 |
| Firmware | UEFI |
| Bootloader | ArchiaOS UEFI loader |
| Kernel | Kernel ArchiaOS freestanding |
| ABI UEFI | Microsoft x64 |
| ABI kernel | SysV AMD64 |
| Compilation | LLVM / Clang |
| Linker | LLD |
| Build system | CMake |
| Virtualisation/test | QEMU + OVMF |
| Firmware graphique | UEFI GOP |
| Tables matérielles | ACPI / SMBIOS |

---

## Chaîne de démarrage

Le bootloader effectue actuellement les étapes suivantes :

1. démarrage UEFI ;
2. découverte du filesystem ;
3. recherche de EFI\ARCHIAOS\ARCHIAOS_kernel.elf ;
4. validation de l'ELF64 x86_64 ;
5. chargement des segments PT_LOAD ;
6. récupération du framebuffer GOP ;
7. découverte ACPI et SMBIOS ;
8. construction du BootInfo ;
9. récupération de la carte mémoire UEFI finale ;
10. appel de ExitBootServices ;
11. handoff vers le kernel x86_64 ;
12. entrée dans kernel_entry puis kernel_main.

Le bootloader utilise l'ABI Microsoft x64 imposée par UEFI et le stub d'handoff adapte les arguments pour le kernel SysV AMD64.

Le chemin ExitBootServices gère également le cas EFI_INVALID_PARAMETER en récupérant une nouvelle memory map avant de réessayer.

---

## Bootloader

### Déjà implémenté

- [x] UEFI x86_64
- [x] chargement ELF64
- [x] validation des headers ELF
- [x] validation des segments PT_LOAD
- [x] validation de l'entry point
- [x] GOP
- [x] ACPI 1.0 / 2.0
- [x] SMBIOS / SMBIOS3
- [x] memory map UEFI
- [x] BootInfo versionné
- [x] ExitBootServices
- [x] handoff assembly
- [x] séparation ABI UEFI / kernel
- [x] réservations mémoire prises en compte côté PMM

Une correction importante a été effectuée dans le chargeur ELF : l'entry point était auparavant lu après la libération de l'image ELF. Les informations nécessaires sont maintenant conservées avant FreePool.

---

## BootInfo

Le bootloader et le kernel partagent une structure BootInfo versionnée.

Elle contient notamment :

- magic/version/taille ;
- adresse et taille de la memory map ;
- taille des descripteurs UEFI ;
- framebuffer, résolution, pitch et format ;
- adresse ACPI RSDP ;
- informations SMBIOS ;
- adresse de base du kernel ;
- taille du kernel ;
- adresse et taille du BootInfo ;
- version du bootloader.

Le kernel vérifie ces informations avant d'initialiser ses sous-systèmes.

---

## Kernel

Le kernel possède maintenant une véritable phase d'initialisation :

    kernel_entry
        ↓
    kernel_main
        ↓
    BootInfo validation
        ↓
    GDT
        ↓
    TSS
        ↓
    IDT (256 vecteurs)
        ↓
    PMM
        ↓
    Paging
        ↓
    Exceptions
        ↓
    LAPIC
        ↓
    Timer
        ↓
    ACPI / MADT

### Initialisation actuellement testée

    ARCHIAOS KERNEL STARTED
    Architecture: x86_64
    BootInfo: OK
    Memory Map: OK
    Framebuffer: OK
    CPU: GDT OK
    CPU: TSS OK
    CPU: IDT 256 VECTORS OK
    PMM TEST PASS
    PAGING TEST PASS
    CPU: INVALID OPCODE HANDLER OK
    IRQ: LAPIC OK
    IRQ: TIMER TEST OK
    ACPI: RSDP/MADT OK

---

## Gestion mémoire

### PMM

Le Physical Memory Manager :

- démarre les frames en état réservé ;
- libère uniquement les types UEFI utilisables ;
- réserve explicitement le kernel ;
- réserve le BootInfo ;
- réserve la memory map ;
- réserve le framebuffer ;
- réserve la page physique 0 ;
- protège contre les débordements de calcul d'adresses ;
- vérifie l'alignement des pages ;
- évite les doubles libérations ;
- possède des tests runtime.

Il possède également un allocateur de pages physiques contiguës, actuellement testé sur quatre pages consécutives avec écriture/lecture réelle.

### Paging

Le paging x86_64 possède maintenant :

- PML4 ;
- PDPT ;
- page directories ;
- pages de 2 MiB ;
- identité virtuelle = physique pour la phase bootstrap ;
- couverture bootstrap jusqu'à 64 GiB ;
- traduction virtuelle → physique ;
- chargement réel de CR3 ;
- validation après activation.

Le test QEMU vérifie notamment :

    PAGING TABLES CREATED
    PAGING IDENTITY MAP PASS (64 GiB)
    PAGING CR3 ACTIVATION PASS
    PAGING TEST PASS

Le paging actuel est volontairement un bootstrap identity map. Il ne constitue pas encore le système de mémoire virtuelle final d'ArchiaOS.

La prochaine évolution sera une architecture de mapping plus complète avec espace noyau, mapping direct de la mémoire physique et gestion propre des zones situées au-dessus de 4 GiB.

---

## CPU, exceptions et interruptions

### GDT / TSS

- GDT initialisée ;
- descripteur TSS construit dynamiquement ;
- TSS chargée ;
- stack d'IST prévue pour les exceptions critiques.

### IDT

Les 256 vecteurs sont initialisés.

Les exceptions possédant un error code matériel sont traitées avec le frame correspondant.

Un test volontaire UD2 vérifie le chemin de l'exception invalid opcode.

### LAPIC

Le Local APIC est initialisé et le timer est fonctionnel sous QEMU.

Le timer génère des interruptions sur le vecteur prévu et le test runtime confirme le fonctionnement.

### Encore incomplet

- IOAPIC complet ;
- routage des IRQ ;
- support x2APIC complet ;
- SMP ;
- AP startup ;
- per-CPU structures ;
- scheduler.

---

## ACPI

Le parseur ACPI actuel sait trouver et analyser :

- RSDP ;
- RSDT ;
- XSDT ;
- MADT ;
- processeurs/APIC ;
- Local APIC ;
- informations IOAPIC.

Test QEMU :

    ACPI: RSDP/MADT OK
    ACPI: CPU/IOAPIC tables parsed

Le routage réel des interruptions via IOAPIC reste à implémenter.

---

## Framebuffer

Le bootloader récupère le framebuffer GOP avant ExitBootServices.

Le kernel vérifie :

- adresse ;
- largeur ;
- hauteur ;
- pitch ;
- taille totale ;
- format.

Un premier rendu graphique est également effectué directement dans le framebuffer.

---

## Tests et validation

La validation principale est effectuée avec :

- QEMU x86_64 ;
- machine q35 ;
- OVMF ;
- debug console sur le port 0xE9.

La CI GitHub :

1. installe Clang/LLD/CMake/QEMU/OVMF ;
2. configure le projet ;
3. compile le kernel et le bootloader ;
4. vérifie les ELF/EFI ;
5. démarre QEMU avec OVMF ;
6. vérifie les marqueurs runtime du kernel.

Les tests ne se contentent donc pas d'un build réussi : le kernel doit réellement démarrer et atteindre les marqueurs attendus.

---

## Arborescence

Le code du projet est regroupé dans le dossier ArchiaOS :

    ArchiaOS/
    ├── boot/
    │   └── uefi/
    │       ├── include/
    │       └── x86_64/
    ├── common/
    │   └── boot_info.h
    ├── kernel/
    │   └── x86_64/
    │       ├── core/
    │       ├── cpu/
    │       └── memory/
    ├── tools/
    │   └── firmware/
    │       └── x86_64/
    │           └── ovmf/
    └── CMakeLists.txt

    .github/
    └── workflows/
        └── archiaos-build.yml

    README.md
    .gitignore

Le dossier .github reste à la racine uniquement parce que GitHub Actions attend les workflows à cet emplacement.

Les fichiers générés restent dans ArchiaOS/build/ et ne sont pas versionnés.

---

## Compilation sous Windows

### 1. Se placer dans le dépôt

    cd C:\Users\Likounsee\Documents\ArchiaOS
    git checkout ArchiaOS
    git pull origin ArchiaOS

Puis entrer dans le projet :

    cd .\ArchiaOS

### 2. Vérifier les outils

    clang++ --version
    ld.lld --version
    cmake --version

Si LLVM est installé dans son emplacement habituel :

    & "C:\Program Files\LLVM\bin\clang++.exe" --version
    & "C:\Program Files\LLVM\bin\ld.lld.exe" --version

### 3. Configuration

    cmake -S . -B build

### 4. Compilation

    cmake --build build --config Release

### 5. Vérifier les artefacts

    Get-Item .\build\ARCHIAOS_kernel.elf
    Get-Item .\build\BOOTX64.EFI
    Get-Item .\build\esp\EFI\BOOT\BOOTX64.EFI
    Get-Item .\build\esp\EFI\ARCHIAOS\ARCHIAOS_kernel.elf

---

## Test QEMU sous Windows

Le test utilise QEMU installé ici :

    C:\Program Files\qemu\qemu-system-x86_64.exe

Depuis :

    cd C:\Users\Likounsee\Documents\ArchiaOS\ArchiaOS

Préparer les variables OVMF :

    Copy-Item .\tools\firmware\x86_64\ovmf\OVMF_VARS.4m.fd .\build\OVMF_VARS.4m.fd -Force

Puis lancer :

    & "C:\Program Files\qemu\qemu-system-x86_64.exe" -machine q35 -m 256M -drive "if=pflash,format=raw,readonly=on,file=tools\firmware\x86_64\ovmf\OVMF_CODE.4m.fd" -drive "if=pflash,format=raw,file=build\OVMF_VARS.4m.fd" -drive "format=raw,file=fat:rw:build\esp" -display none -serial none -monitor none -debugcon stdio -global isa-debugcon.iobase=0xe9 -no-reboot -no-shutdown

Les messages du kernel doivent apparaître directement dans PowerShell.

Pour arrêter QEMU : Ctrl+C.

### Nettoyage d'un build

    Remove-Item -Recurse -Force .\build
    cmake -S . -B build
    cmake --build build --config Release

Ne supprime pas novos.vhdx : ce fichier n'est pas nécessaire pour ce nettoyage.

---

## CI GitHub

Le workflow se trouve volontairement à :

    .github/workflows/archiaos-build.yml

Il construit désormais le projet depuis :

    ArchiaOS/

et génère :

    ArchiaOS/build/

Les validations QEMU vérifient notamment :

- démarrage du kernel ;
- BootInfo ;
- memory map ;
- framebuffer ;
- GDT ;
- TSS ;
- IDT ;
- PMM ;
- paging ;
- invalid opcode ;
- LAPIC ;
- timer ;
- ACPI.

---

## Avancement

Les pourcentages ci-dessous sont des estimations d'ingénierie, pas des métriques officielles.

| Sous-système | Avancement |
|---|---:|
| Bootloader UEFI | ~90 % |
| ELF loader | ~90 % |
| Filesystem UEFI | ~80 % |
| BootInfo / ABI | ~90 % |
| ExitBootServices | ~95 % |
| GOP / framebuffer | ~85 % |
| Kernel bootstrap | ~80 % |
| GDT | ~85 % |
| TSS | ~80 % |
| IDT | ~80 % |
| Exceptions | ~65 % |
| PMM | ~75 % |
| Paging bootstrap | ~60 % |
| Mémoire virtuelle complète | ~20 % |
| LAPIC | ~60 % |
| IRQ | ~55 % |
| ACPI | ~55 % |
| IOAPIC | ~25 % |
| x2APIC | ~0 % |
| SMP | ~0 % |
| Heap kernel | ~0 % |
| Scheduler | ~0 % |
| Drivers | ~5 % |
| VFS | ~0 % |
| Storage | ~0 % |
| Processus | ~0 % |
| User mode | ~0 % |
| Syscalls | ~0 % |
| Shell | ~0 % |
| GUI | ~0 % |

### Vue globale

**Fondations boot + kernel : ~75 %**

**OS complet : ~25 %**

Ces valeurs représentent l'état technique du projet et non un pourcentage mathématique du nombre total de fonctionnalités possibles.

---

## Prochaines étapes

### Mémoire

- [x] PMM
- [x] allocation de pages
- [x] allocation contiguë
- [x] bootstrap paging
- [x] activation CR3
- [ ] HHDM / direct physical map
- [ ] virtual address manager
- [ ] kernel heap
- [ ] protections mémoire
- [ ] page fault avancée

### Interruptions / CPU

- [x] GDT
- [x] TSS
- [x] IDT 256 vecteurs
- [x] exceptions
- [x] LAPIC
- [x] timer
- [ ] IOAPIC routing
- [ ] x2APIC
- [ ] SMP
- [ ] per-CPU data
- [ ] AP startup

### Kernel

- [ ] scheduler
- [ ] threads
- [ ] context switching
- [ ] processus
- [ ] espaces d'adresses séparés
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
- [ ] serveur graphique
- [ ] GUI
- [ ] applications natives

---

## Principes de développement

1. Ne jamais supposer qu'une structure fournie par le firmware est valide.
2. Vérifier les débordements et les bornes physiques.
3. Garder le bootloader et le kernel découplés par un ABI versionné.
4. Ne plus appeler les Boot Services après un ExitBootServices réussi.
5. Tester chaque sous-système critique dans QEMU avant de le considérer comme stable.
6. Éviter les optimisations prématurées qui rendent le kernel plus difficile à vérifier.
7. Nettoyer le code lorsqu'une simplification ne change pas le comportement attendu.
8. Ne jamais considérer un simple build réussi comme une validation runtime.

---

## Licence

Aucune licence open source n'a encore été choisie pour ArchiaOS.
