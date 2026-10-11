# Stratégie de tests

La CI compile le noyau pour `x86_64-unknown-none`, vérifie que les segments ELF sont placés en espace virtuel haut, crée une ISO Limine et démarre cette ISO dans QEMU. Le test de démarrage exige le message série d’entrée noyau et le message de validation de la carte mémoire. Le build seul ne prouve pas le démarrage ; seul le résultat de l’étape QEMU valide ce smoke test.

Vérification manuelle depuis `ArchiaOS` :

```sh
rustup target add x86_64-unknown-none
cargo build --release
scripts/make-iso.sh
qemu-system-x86_64 -cdrom build/archiaos.iso -boot d -serial stdio -display none -no-reboot -no-shutdown
```

La carte mémoire est vérifiée pour les pointeurs nuls, le nombre d’entrées, les longueurs nulles et les débordements d’adresses/sommes. Ces contrôles ne remplacent pas encore des tests unitaires d’allocateur, qui n’existe pas encore.
