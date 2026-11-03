# ADR-0001 : analyser et archiver le G-code par machine

**Status:** Accepted  
**Date:** 2026-11-03  
**Deciders:** Mainteneur FabLabCAM, dans le cadre de l'issue [#9](https://github.com/FabLabCAM/core/issues/9).

## Contexte

Le contrôle dépend du dialecte et des réglages réels de la machine. Le backplot
existant est une visualisation et ne fournit pas une validation exhaustive.
Conserver les sept crates et les API de génération existantes est impératif.

## Décision

- Moteur pur dans `fablabcam_cam::gcode::machine`, fichiers dans
  `fablabcam_studio::gcode_store`, adaptation CLI `fablabcam machine`.
- Profils JSON typés et versionnés, sérialisés avec Serde. Les paramètres inconnus
  sont refusés pour détecter les fautes de configuration.
- Analyse littérale en double précision, diagnostics ligne/colonne (base 1).
  Les erreurs n'empêchent pas d'examiner les lignes suivantes. Une construction
  non interprétée rend le rapport incomplet ; les statistiques sont alors partielles.
- Conversion limitée à des programmes dont la sémantique est connue : normalisation
  de format et alias de codes équivalents ; modes initiaux explicites. Validation avant et après.
- Archives locales immuables par révision, profil et rapport inclus, auteur,
  horodatage et SHA-256. Verrou de fichier pour sérialiser les écritures.

## Options considérées

| Option | Complexité / coût | Échelle | Familiarité | Avantages / limites |
| --- | --- | --- | --- | --- |
| Backplot existant | Faible | Locale | Forte | Réemploi, mais tolère des mots ignorés et ne modélise pas tous les plans |
| `gcode` 0.7 | Moyenne | Locale | Nouvelle | Bibliothèque MIT/Apache ; valeurs f32, numéros G limités à une décimale, perte des blocs partiellement invalides dans l'AST |
| Analyseur littéral dédié + Serde | Moyenne | Fichiers locaux | Rust existant | Positions et précision maîtrisées ; sous-ensemble explicitement documenté à maintenir |
| Base de données / service | Élevée | Multiutilisateur | Nouvelle | Transactions centrales, mais déploiement et dépendances inutiles pour ce besoin |

## Compromis

La bibliothèque `gcode` a été évaluée dans ses sources 0.7.0. Ses limites de
précision et de conservation des tokens ne conviennent pas au contrôle de course.
Un petit analyseur de mots littéraux est donc retenu. Il n'exécute ni expressions,
ni macros, ni programmes de contrôleur. JSON suffit au CRUD demandé ; YAML n'ajoute
pas de capacité utile à cette première version.

## Conséquences

Le profil Mach3 **Mill** se réfère au [manuel officiel 1.84-A2](https://www.machsupport.com/wp-content/uploads/2013/02/Mach3Mill_1.84.pdf),
chapitre 10. Il ne certifie ni une version du contrôleur ni une machine physique.
Les courses, vitesses et positions initiales doivent être configurées localement.
Les limites de cette analyse et l'exclusion des collisions restent visibles.

## Actions

- [x] Implémenter profils, analyse, validation, conversion et archives.
- [x] Tester les limites, arcs, modes, diagnostics et révisions concurrentes.
- [ ] Publier exemples, guide, notes de version et installateurs de la v0.3.0.
- TODO(agent) : étendre les cycles et transformations avec des modèles explicites,
  sans assimiler une commande reconnue à une commande interprétée.
