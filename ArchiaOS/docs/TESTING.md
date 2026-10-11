# Stratégie de tests

La CI compile le noyau pour `x86_64-unknown-none`. Cela vérifie la compilation et la liaison, mais ne prouve pas le démarrage réel ni le fonctionnement des registres CPU.

Vérification manuelle proposée depuis `ArchiaOS` :

```sh
rustup target add x86_64-unknown-none
cargo build --release
qemu-system-x86_64 -kernel target/x86_64-unknown-none/release/archiaos-kernel -serial stdio -display none -no-reboot -no-shutdown
```

Le test doit observer les lignes de diagnostic sur COM1. Cette commande reste à exécuter et son résultat à consigner. Ajouter ultérieurement un délai d’arrêt contrôlé dans le script CI et vérifier l’en-tête Multiboot avec les outils GRUB lorsque disponibles.

Il n’existe pas encore de tests unitaires mémoire ou interruptions : ces composants ne sont pas implémentés. `cargo test` seul ne valide pas le démarrage bare-metal.