# Interruptions et exceptions

## État réel

Le noyau installe une IDT x86-64 de 256 entrées. Chaque vecteur dispose d'un stub ISR et la table des adresses est utilisée pour construire les gates. Les stubs normalisent la pile en tenant compte des exceptions où le processeur pousse déjà un code d'erreur. L'entrée commune transmet le vecteur, le code d'erreur et le RIP au dispatcher selon l'ABI attendue.

Le dispatcher actuel est fatal et non retournant. Il journalise les exceptions et peut lire CR2 pour diagnostiquer une faute de page, puis s'arrête. Il n'existe pas encore de chemin général de sauvegarde/restauration de contexte suivi d'un retour iretq. Il ne faut donc pas traiter cet état comme un mécanisme de gestion d'exceptions récupérables.

## IRQ matérielles et PIC legacy

Le PIC 8259 est initialisé en restant masqué. L'IDT contient les vecteurs de la plage remappée, mais cela ne signifie pas que les IRQ matérielles sont activées ni qu'elles sont traitées par des handlers fonctionnels. La voie d'IRQ non prise en charge reste fatale ; le noyau ne doit pas démasquer les lignes avant que le dispatch, l'acquittement EOI et les chemins d'erreur soient validés.

## Validation

Le script scripts/verify-isr-stubs.py audite statiquement la couverture des 256 stubs, leur absence de doublons, la normalisation des codes d'erreur, l'ordre de la table, les instructions d'entrée commune, le contrat du dispatcher, les champs et tailles IDT/IDTR et l'ordre d'installation. La CI exécute ce script, mais son succès doit être vérifié sur un run effectif. Les tests statiques ne remplacent pas les scénarios QEMU ni une validation sur matériel.

## Travail restant

- Exécuter et maintenir l'audit statique et les tests CI.
- Ajouter des scénarios QEMU reproductibles pour les exceptions représentatives, dont une faute de page contrôlée.
- Concevoir un vrai chemin de retour seulement après la sauvegarde complète de l'état CPU, le traitement des piles et les règles de privilège.
- Garder les IRQ masquées jusqu'à ce que les handlers, EOI, temporisation et scénarios d'échec soient testés.
