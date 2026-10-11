# Stratégie de tests

La CI compile le noyau pour `x86_64-unknown-none`, vérifie les adresses virtuelles ELF, crée une ISO Limine et démarre cette ISO dans QEMU. Le test exige les messages de l’entrée noyau, de validation de la carte mémoire et de l’auto-test PMM.

Le test PMM alloue deux pages gérées et distinctes, vérifie l’alignement et la limite 4 Gio, confirme la baisse du compteur libre de deux, libère les pages, vérifie le rétablissement du compteur et exige que les libérations de zéro, d’une adresse non alignée, d’une page réservée et de la limite exclusive soient rejetées, ainsi que le double-free.

Vérification manuelle depuis `ArchiaOS` :

```sh
rustup target add x86_64-unknown-none
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

Ce smoke test ne remplace pas des tests de fragmentation, de concurrence, de réservations chevauchantes ou de gestion de fautes ; ils restent nécessaires avant un usage général.
