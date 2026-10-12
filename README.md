# ArchiaOS

**ArchiaOS est un système d'exploitation expérimental en cours de construction.** Le noyau de production x86-64 démarre encore avec Limine. Le bootloader UEFI maison a réussi des transferts vers un noyau de test x86-64 sous OVMF/QEMU et vers le prototype ARM64 sous AAVMF/QEMU.

## État confirmé par la CI

- Compilation et smoke test du noyau de production x86-64 via Limine.
- Tests unitaires des parseurs BootInfo, ELF64, ACPI, Device Tree et carte mémoire.
- Compilation du bootloader maison pour UEFI x86-64 et AArch64.
- Transfert UEFI x86-64 validé sous OVMF/QEMU avec un noyau de test identité.
- Transfert UEFI ARM64 validé sous AAVMF/QEMU, avec découverte de la console via ACPI SPCR, validation de BootInfo/carte mémoire, auto-test PMM et exception synchrone piégée.
- Démarrage direct du prototype noyau AArch64 sous QEMU `virt`.

Cela ne signifie pas encore que le noyau de production high-half peut démarrer sans Limine, ni que toutes les plateformes ARM64 sont compatibles. Le chargeur actuel exige des segments ELF identité, alignés sur page et sous 4 Gio. Le firmware UEFI standard reste fourni par la machine.

## Documentation

- [Documentation technique et compilation](ArchiaOS/README.md)
- [Roadmap](ArchiaOS/docs/ROADMAP.md)
- [Architecture bootloader et ARM64](ArchiaOS/docs/BOOT_AND_ARCHITECTURES.md)
- [Prototype UEFI](ArchiaOS/bootloader/README.md)
- [Prototype noyau AArch64](ArchiaOS/kernel/aarch64/README.md)

Le développement se fait exclusivement sur la branche **`ArchiaOS`**.
