# ArchiaOS

**ArchiaOS est un système d'exploitation expérimental en cours de construction.** Le noyau x86-64 de production démarre encore avec Limine. Le projet développe son propre bootloader UEFI et prépare l'exécution ARM64 dans le même dépôt, sur la branche `ArchiaOS`.

## État du développement

- **Bootloader maison :** lecture du noyau depuis la partition EFI, validation ELF64, allocation et chargement de segments identité sous 4 Gio, initialisation du BSS, contrat BootInfo versionné, carte mémoire finale et sortie des services UEFI.
- **Handoff x86-64 :** une image de test indépendante démarre sous OVMF/QEMU et vérifie BootInfo. Le noyau de production, lié en high-half et dépendant de Limine, n'est pas encore basculé vers ce chargeur.
- **ARM64/AArch64 :** le bootloader charge désormais les segments identité, synchronise le cache d'instructions, récupère le Device Tree UEFI et transmet BootInfo au noyau AArch64. Le noyau vérifie l'ABI, le header FDT et les descripteurs mémoire. Le test AAVMF/QEMU complet est ajouté à la CI ; sa réussite reste à confirmer.
- **Firmware :** le firmware UEFI standard de la carte mère ou de la machine virtuelle reste utilisé ; aucun bootloader tiers n'est prévu dans la chaîne finale.

La compilation de l'application UEFI AArch64 ou du noyau ne suffit pas à démontrer la compatibilité de l'ensemble de l'OS. Il faut encore valider le démarrage complet en CI, puis ajouter les exceptions, la MMU, les timers, les interruptions et les pilotes ARM64. Le stockage, la VFS et l'intégration OpenFS sont des étapes ultérieures.

## Documentation

- [Documentation technique et commandes de build](ArchiaOS/README.md)
- [Roadmap et critères d'acceptation](ArchiaOS/docs/ROADMAP.md)
- [Architecture du bootloader et ARM64](ArchiaOS/docs/BOOT_AND_ARCHITECTURES.md)
- [Prototype UEFI](ArchiaOS/bootloader/README.md)
- [Prototype noyau AArch64](ArchiaOS/kernel/aarch64/README.md)

Le développement se fait exclusivement sur la branche **`ArchiaOS`** ; `main` ne doit pas être modifiée.
