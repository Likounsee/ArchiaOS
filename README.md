# ArchiaOS

**ArchiaOS est un système d'exploitation expérimental en cours de construction.** Le noyau x86-64 Rust `no_std` démarre actuellement avec Limine et est testé sous QEMU. Le développement d'un bootloader UEFI maison et la préparation d'ARM64 ont commencé sur la branche `ArchiaOS`.

## État du projet

- Noyau x86-64 : entrée, sortie série, validation de la carte mémoire et allocateur bitmap physique expérimental testés en QEMU.
- Bootloader maison : prototype Rust UEFI sans dépendance externe, qui affiche un diagnostic. Il ne charge pas encore le noyau et ne remplace pas Limine.
- ARM64 : prototype de noyau AArch64 distinct avec entrée assembleur et console PL011 pour QEMU virt ; la CI vérifie sa compilation, mais le démarrage en exécution reste à valider.
- CI : tests de démarrage x86-64 existants et compilations des cibles UEFI x86-64/AArch64 et du prototype noyau AArch64.

Le firmware UEFI standard de la machine reste utilisé. La compilation du prototype ARM64 ne signifie pas encore que l'ensemble du noyau ArchiaOS est compatible avec ARM64. Le bootloader maison ne deviendra le chemin par défaut qu'après un chargement de noyau validé de bout en bout.

## Documentation

- [Documentation technique et commandes de build](ArchiaOS/README.md)
- [Roadmap et critères](ArchiaOS/docs/ROADMAP.md)
- [Architecture bootloader et ARM64](ArchiaOS/docs/BOOT_AND_ARCHITECTURES.md)
- [Prototype de bootloader UEFI](ArchiaOS/bootloader/README.md)
- [Prototype noyau AArch64](ArchiaOS/kernel/aarch64/README.md)

Le développement se fait exclusivement sur la branche **`ArchiaOS`** ; `main` ne doit pas être modifiée.
