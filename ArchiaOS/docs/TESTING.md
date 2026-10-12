# Stratégie de tests

La validation du démarrage est centrée sur le chargeur UEFI ArchiaOS. La CI compile le noyau x86-64 par défaut avec `uefi-boot`, compile les applications UEFI x86-64 et AArch64, exécute les tests des parseurs partagés, puis démarre le chargeur et les noyaux sous OVMF/QEMU et AAVMF/QEMU.

## Couverture automatisée

- Compilation du noyau x86-64 et contrôle des adresses virtuelles ELF high-half.
- Tests unitaires BootInfo, ELF64, ACPI, Device Tree et carte mémoire.
- Compilation du chargeur UEFI x86-64 et AArch64.
- Démarrage x86-64 avec le chargeur maison, y compris le noyau de production, un noyau identité et un noyau high-half ; vérification du transfert, de BootInfo, de la carte mémoire, du PMM et des protections de pages.
- Cas négatifs UEFI : ELF invalide, ELF valide demandant une plage physique indisponible, fichier noyau absent et BootInfo invalide injecté uniquement dans un build de test.
- Compilation et démarrage direct du prototype AArch64, plus démarrage UEFI AArch64 sous AAVMF/QEMU.

Les tests négatifs doivent confirmer non seulement le diagnostic attendu, mais aussi l'absence de transfert au noyau lorsqu'une erreur est détectée avant le handoff. Le workflow dédié BootInfo invalide exige que le noyau reçoive le handoff puis rejette le contrat ; il échoue si le noyau déclare BootInfo valide.

## Commandes locales

Depuis le dossier `ArchiaOS` :

```sh
rustup target add x86_64-unknown-linux-gnu x86_64-unknown-none x86_64-unknown-uefi aarch64-unknown-uefi
cargo test --manifest-path bootloader/Cargo.toml --lib --target x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-none
cargo build --manifest-path bootloader/Cargo.toml --release --target x86_64-unknown-uefi
cargo build --manifest-path bootloader/Cargo.toml --release --target aarch64-unknown-uefi
```

Le test d'injection BootInfo invalide est réservé au chargeur de test :

```sh
cargo build --manifest-path bootloader/Cargo.toml --release --features test-invalid-bootinfo --target x86_64-unknown-uefi
```

Ne jamais utiliser cette feature pour une image normale : elle corrompt intentionnellement le magic du BootInfo pour vérifier le rejet par le noyau.

## Validation matérielle P0

Les VM ne remplacent pas un essai sur machine physique. Lorsqu'une machine de test est disponible :

- [ ] Identifier la machine, le CPU et la version du firmware UEFI.
- [ ] Préparer un support EFI de test distinct et sauvegarder toute donnée utile.
- [ ] Confirmer le mode UEFI natif et noter l'état Secure Boot ; les binaires de développement non signés peuvent être refusés.
- [ ] Démarrer le chargeur ArchiaOS et conserver les diagnostics disponibles.
- [ ] Vérifier le handoff, la validation BootInfo, la carte mémoire et le test PMM.
- [ ] Faire un redémarrage à froid et vérifier que le chemin de récupération du firmware reste accessible.
- [ ] Documenter les échecs et ne déclarer que cette configuration testée, sans extrapoler à tout le matériel.

## Historique et état de validation

Les exécutions liées au HEAD `1ec20483c4e699ae884a271d4b1613531d158070` ont historiquement passé les tests complets [QEMU/OVMF + AAVMF](https://github.com/Likounsee/ArchiaOS/actions/runs/38247434624) et le [rejet BootInfo invalide](https://github.com/Likounsee/ArchiaOS/actions/runs/38247434330). Ces résultats concernent ce commit uniquement et ne valident pas les changements plus récents.

Sur le HEAD `1802e427fa7552dc3062f313a871c64d4f42cea4`, le workflow de packaging EFI physique a réussi, mais la CI principale était en échec. Après les contrôles d’accessibilité ajoutés en P1, une nouvelle exécution de la CI principale a également échoué ; les workflows de démarrage étaient encore en cours au dernier relevé. Consulte les [exécutions de la branche ArchiaOS](https://github.com/Likounsee/ArchiaOS/actions?query=branch%3AArchiaOS) pour le résultat le plus récent avant de considérer ces changements validés.

Le premier démarrage et l’écriture framebuffer ont été confirmés par l’utilisateur sur une machine physique. Ce résultat manuel ne remplace pas les tests CI et ne prouve pas la compatibilité universelle.

## Limites des tests

Le smoke test ne couvre pas toutes les variantes de firmware, les erreurs de mémoire matérielles, la fragmentation et la concurrence approfondies, ni toutes les réservations de mémoire. Ces audits supplémentaires appartiennent à des étapes ultérieures ; ils ne doivent pas être présentés comme validés par le simple succès de QEMU.
