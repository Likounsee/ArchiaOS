# Démarrage

Le noyau cible le protocole natif **Limine** et un ELF64 x86-64 lié dans la moitié haute de l’espace virtuel. Le script de liaison sépare les adresses virtuelles et les adresses physiques de chargement et garde les requêtes Limine dans un segment accessible en écriture.

Le script `scripts/make-iso.sh` télécharge Limine v12.9.1, vérifie le SHA-256, crée une ISO BIOS/UEFI et exécute `limine bios-install`. La CI démarre cette ISO sous QEMU. L’entrée Rust et la sortie COM1 sont vérifiées, tout comme la validation de la carte mémoire et l’auto-test de l’allocateur bitmap.

Le point d’entrée assembleur aligne la pile puis appelle Rust selon l’ABI x86-64 SysV. Le noyau ne traite pas encore les exceptions/IRQ et laisse les interruptions désactivées.
