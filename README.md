# ArchiaOS

**ArchiaOS est un système d'exploitation expérimental en cours de construction.** Le firmware UEFI standard de la machine reste utilisé ; le bootloader est désormais développé par ArchiaOS. Le chemin de démarrage par défaut du noyau x86-64 utilise maintenant le bootloader UEFI développé par ArchiaOS. Le noyau principal reçoit le contrat BootInfo maison et initialise son allocateur physique depuis la carte mémoire UEFI.

## État confirmé par la CI

- Compilation du noyau principal x86-64 avec l’entrée UEFI maison par défaut.
- Démarrage du vrai noyau x86-64 sous OVMF/QEMU via le chargeur ArchiaOS et auto-test PMM de 130 pages (test CI dédié).
- Tests des parseurs BootInfo, ELF64, ACPI, Device Tree et carte mémoire.
- Compilation du bootloader maison pour UEFI x86-64 et AArch64.
- Démarrage UEFI du noyau de production x86-64 high-half avec BootInfo UEFI et auto-test PMM réussi.
- Contrôle x86-64 des permissions de page W^X/NX : code high-half en lecture seule/exécutable, données RW/NX, alias physique du code RO/NX, CR0.WP et EFER.NXE vérifiés.
- Démarrage UEFI ARM64 sous AAVMF/QEMU, avec console PL011 découverte via ACPI SPCR, validation BootInfo/carte mémoire, auto-test PMM et exception synchrone piégée.
- Démarrage direct du prototype noyau AArch64 sous QEMU `virt`.

## Limites actuelles

Les adresses physiques chargées, les buffers de transfert et le chargeur x86-64 doivent être dans la fenêtre identité sous 4 Gio ; le fichier noyau est limité à 64 Mio et les segments cumulés à 128 Mio. Le chemin high-half testé concerne x86-64. Le prototype ARM64 utilise encore les mappings du firmware et n'a pas une MMU propre au noyau. Les tests valident des configurations QEMU, pas tout le matériel réel. Limine demeure la voie de démarrage standard jusqu'à validation élargie.

## Documentation

- [Documentation technique et compilation](ArchiaOS/README.md)
- [Roadmap](ArchiaOS/docs/ROADMAP.md)
- [Architecture bootloader et ARM64](ArchiaOS/docs/BOOT_AND_ARCHITECTURES.md)
- [Prototype UEFI](ArchiaOS/bootloader/README.md)
- [Prototype noyau AArch64](ArchiaOS/kernel/aarch64/README.md)

Le développement se fait exclusivement sur la branche **`ArchiaOS`**.
