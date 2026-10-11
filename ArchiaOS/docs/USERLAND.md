# Espace utilisateur

Il n’existe ni transition vers le ring 3, ni processus isolés, ni ABI syscall, ni chargeur ELF utilisateur, ni runtime, ni shell. Le noyau tourne en privilège noyau et s’arrête dans une boucle `hlt`.

L’espace utilisateur dépend d’une IDT fiable, d’une gestion mémoire virtuelle avec protections, d’un modèle de contexte et d’une interface syscall documentée. Une application ne devra être annoncée comme prise en charge qu’après chargement, isolation et exécution d’un véritable programme utilisateur.