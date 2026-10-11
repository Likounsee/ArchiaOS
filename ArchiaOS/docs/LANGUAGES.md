# Choix des langages

- **Rust stable (`no_std`)** est utilisé pour le noyau après l’entrée fournie par Limine. La cible est `x86_64-unknown-none`; le noyau utilise `panic = abort` et n’a pas de runtime standard.
- **Assembleur x86** n’est plus nécessaire à la transition de mode initiale puisque Limine fournit l’entrée en mode long. Il pourra être ajouté pour les stubs d’interruptions ou le changement de contexte lorsque ces composants seront réellement implémentés.
- **C et C++ ne sont pas introduits** : aucun code existant ni ABI ne les exige dans cette tranche.
- **Python** pourra servir aux outils de génération et de test.
- **Langages web** ne sont pertinents qu’une fois qu’un runtime utilisateur existera.

Les instructions privilégiées et le port I/O nécessitent `unsafe`. Chaque bloc doit documenter ses préconditions. Rust ne supprime pas les risques liés aux tables de pages, aux registres CPU, aux interruptions ou au code assembleur éventuel.
