# Choix des langages

- **Rust stable (`no_std`)** est utilisé pour le noyau après l’entrée CPU. La cible est `x86_64-unknown-none`; le noyau utilise `panic = abort` et n’a pas de runtime standard.
- **Assembleur x86** est nécessaire à la transition du mode protégé 32 bits vers le mode long 64 bits, aux tables de pages initiales et à la pile de démarrage. Il ne doit pas contenir de logique générale.
- **C et C++ ne sont pas introduits** : aucun code existant ni ABI ne les exige dans cette tranche. Ils ne seront ajoutés que pour un besoin d’intégration documenté.
- **Python** pourra servir aux outils de génération et de test.
- **Langages web** ne sont pas pertinents au niveau noyau et ne seront considérés que lorsqu’un runtime utilisateur existera.

Les instructions privilégiées et le port I/O nécessitent `unsafe`. Chaque bloc doit documenter ses préconditions. Rust ne supprime pas les risques liés aux tables de pages, aux registres CPU, aux interruptions ou à l’assembleur. Les options du build désactivent la red zone et fixent le modèle de code kernel.