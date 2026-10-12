# ArchiaOS

**ArchiaOS est un système d'exploitation expérimental en cours de construction.** La machine conserve son firmware UEFI standard ; le chargeur est développé par ArchiaOS. Le chemin par défaut du noyau x86-64 utilise le chargeur UEFI maison et le contrat BootInfo versionné.

## État technique

- Le noyau x86-64 par défaut compile avec l'entrée UEFI ArchiaOS.
- La CI compile les chargeurs UEFI x86-64 et AArch64 et exécute les tests des parseurs BootInfo, ELF64, ACPI, Device Tree et carte mémoire.
- Le démarrage du noyau de production x86-64 sous OVMF/QEMU valide le transfert UEFI, BootInfo, la carte mémoire et l'auto-test PMM.
- Le chemin x86-64 high-half teste les permissions W^X/NX, CR0.WP et EFER.NXE.
- Un workflow dédié a validé le rejet d'un BootInfo volontairement invalide après le handoff UEFI.
- Les tests négatifs UEFI couvrent ELF invalide, plage physique indisponible et noyau absent.
- Le prototype noyau AArch64 se compile et démarre en QEMU direct ; le transfert UEFI AArch64 est testé sous AAVMF/QEMU, avec console PL011, BootInfo/carte mémoire, PMM et une exception synchrone contrôlée.

Les résultats sont propres aux configurations virtuelles exécutées par la CI. Le statut du dernier ensemble CI doit être consulté dans [GitHub Actions](https://github.com/Likounsee/ArchiaOS/actions/workflows/ci.yml) ; une exécution encore en cours ne doit pas être présentée comme entièrement verte.

## Limites actuelles

Le chemin de chargement testé impose des contraintes de mémoire physique sous 4 Gio ; le fichier noyau est limité à 64 Mio et les segments cumulés à 128 Mio. Le chargement high-half est pris en charge sur x86-64, tandis que le chemin ARM64 utilise encore des segments identité et n'a pas de transition MMU propre au noyau. La validation sur matériel physique reste nécessaire pour chaque configuration revendiquée. Le chemin Limine a été retiré du démarrage opérationnel.

## Documentation

- [Documentation technique et compilation](ArchiaOS/README.md)
- [Roadmap P0–P10](ArchiaOS/docs/ROADMAP.md)
- [Démarrage UEFI](ArchiaOS/docs/BOOT.md)
- [Stratégie de tests](ArchiaOS/docs/TESTING.md)
- [Architecture du bootloader et ARM64](ArchiaOS/docs/BOOT_AND_ARCHITECTURES.md)
- [Implémentation du bootloader](ArchiaOS/bootloader/README.md)
- [Prototype noyau AArch64](ArchiaOS/kernel/aarch64/README.md)

Le développement se fait exclusivement sur la branche **`ArchiaOS`**.
