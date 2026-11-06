# ADR 0006 — Rapports de panne et session de diagnostic (#24)

## Architecture

Le calcul d'offset reste dans `fablabcam-geom`. Les rapports et leur configuration
résident dans `fablabcam-studio::diagnostics`, sans dépendance à egui. L'interface
appelle ce service depuis Préférences ; son initialisation précède le chargement
du projet et la création de la fenêtre. Les sept crates sont conservés.

## Composants

- Journal JSONL borné, événements métier explicites : aucune journalisation
  globale des clients réseau, des clés IA ou du contenu des projets.
- Rapports de panique séparés et créés exclusivement : une seconde panique ne
  remplace pas la première. Le chemin fatal n'utilise pas le verrou du journal.
- Réglages persistants du niveau et des traces ; armement du core limité à la
  session, jamais mémorisé pour les démarrages suivants.
- Backend macOS : préconditions du noyau et du système de fichiers, limite
  RLIMIT_CORE du processus. Aucun sysctl ni réglage launchd global modifié.
- Export de rapports texte/JSONL seulement ; les dumps mémoire restent locaux.

## Code

Utiliser la bibliothèque standard (`panic::set_hook`, `Backtrace::force_capture`),
serde_json et tempfile déjà présents. Un collecteur de logs réseau général n'est
pas nécessaire pour ces événements de calcul et risquerait d'enregistrer des
données de l'assistant. Le gestionnaire de panique doit conserver les diagnostics
stderr, ne pas appeler egui et ne pas dépendre des verrous du moteur.

Les réglages changent des indicateurs atomiques du service, pas les variables
d'environnement d'un processus multithread. En mode core seulement, écrire et
fermer le rapport avant `abort`. Un état « armé » décrit des préconditions
vérifiées, pas une garantie de dump : macOS peut encore le refuser.

Sur macOS récent, la signature doit inclure `get-task-allow`. Le Studio vérifie
`CS_GET_TASK_ALLOW` avant de proposer l'armement ; les paquets normaux ne gagnent
pas cette permission. `packaging/diagnostic_macos.py` crée un bundle distinct,
optimisé avec symboles/dSYM, signé pour ce diagnostic uniquement. Garder ensemble
son binaire exact, son dSYM et le core ; comparer les UUID avec `dwarfdump`.

XNU compare `RLIMIT_CORE` à la taille **virtuelle** du processus : un simple test
C réserve ici 445 Go, mais son core occupe 5,5 Go. Un plafond de 1 Gio empêche
silencieusement toute capture. Le service utilise donc une limite virtuelle
finie de 1 Tio, exige 10 Gio libres et explique que cela ne borne pas strictement
l'occupation disque. Il ne change que la limite souple du processus, rétablie au
désarmement. Les autorisations du dossier, le noyau, launchd et les entitlements
ne sont jamais modifiés par l'interface. Le mode core transforme aussi une
panique normalement récupérable en arrêt volontaire après écriture du rapport.

Références : [Apple DTS](https://developer.apple.com/forums/thread/694233),
[XNU kern_core.c](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_core.c).
Les erreurs natives, l'épuisement mémoire et SIGKILL ne passent pas nécessairement
par le gestionnaire Rust ; un rapport de panique n'est pas garanti pour ces cas.

Les journaux utilisent huit emplacements verrouillés par le système : 1 Mio par
fichier et trois rotations par emplacement, soit au plus 32 Mio. Le verrou
survit aux rotations, pas au processus. Les vingt derniers rapports de panique
sont conservés au démarrage ; aucune rotation n'est faite dans le hook fatal.
L'export énumère explicitement les journaux/rapports, exclut les liens symboliques,
les préférences, les projets et les cores. Les rapports peuvent toutefois citer
un message ou un chemin privé : relecture nécessaire avant partage.

## Comment tester

Régression géométrique avant/après, DXF réel en QA locale, propriétés des parcours,
panique dans un sous-processus, rapports successifs, réglages/export/rotation,
refus d'armement et restauration de limite, vérification du signal d'abandon.
Valider l'interface avec des données de démonstration et des dossiers isolés.

## Prochaines améliorations

TODO(agent) : backends de dumps Linux/Windows ; conserver leurs options explicites
et ne pas annoncer une capture native sans vérification. La correction du calcul
et la production des diagnostics sont deux responsabilités distinctes.
