# Architecture de FabLabCAM/core

## Décision

Un workspace Cargo regroupe sept crates. `core` est le nom du dépôt, pas une
crate supplémentaire. La géométrie et le modèle de document alimentent le moteur
CAM ; le Studio orchestre les opérations. Le backplot relit le G-code émis pour
vérifier les trajectoires réellement exportées. CLI et egui utilisent ces moteurs.

## Identité

Produit : FabLabCAM. Éditeur : FabLabCAM Studio. Exécutables : `fablabcam`,
`fablabcam-view`, `fablabcam-studio-ui`. Identifiants d'application :
`org.fablabcam.studio` et `org.fablabcam.view`.

Le dépôt initial contient uniquement les sources et exemples destinés à la
publication. Les archives historiques, captures antérieures et programmes
externes restent hors du dépôt. Les tests XML conservent les projets de dessin
fournis par le mainteneur ; les comparaisons avec des sorties propriétaires ont
été remplacées par des contrôles de géométrie et de profondeur. Ils ne prouvent
pas l'équivalence avec une autre application.

## Préférences

`FABLABCAM_PREFS` remplace le chemin du fichier de préférences. Par défaut :
`~/Library/Application Support/FabLabCAM/preferences.txt` sur macOS,
`%APPDATA%/FabLabCAM/preferences.txt` sur Windows,
`$XDG_CONFIG_HOME/fablabcam/preferences.txt` (ou `~/.config/fablabcam/`) sur Linux.
Les préférences d'une installation précédente ne sont pas migrées automatiquement.

## Limites

Le moteur traite la FAO 2,5D. La simulation discrétise le brut en grille de
hauteurs ; ce n'est pas une simulation cinématique complète d'une machine.
L'interface web exportée est une visualisation, pas un éditeur équivalent à egui.
Le format de sauvegarde est décrit dans `file-formats.md`.

La hauteur du texte utilise la métrique des capitales de la police. Si cette
métrique est absente, elle est mesurée sur le glyphe `H` ; l'estimation à 70 % du
cadratin reste le dernier recours pour les polices sans cette lettre.

## Gestion des G-code par machine

Le module pur `fablabcam_cam::gcode::machine` analyse et valide les programmes ;
`fablabcam_studio::gcode_store` conserve profils et révisions. La CLI les expose
sans ajouter de crate. Voir l’[ADR-0001](adr/0001-machine-gcode.md).

## Édition de mobilier composé

Le domaine `furniture` sépare `composition` (arbre, épaisseurs, orientations,
validation et copie), `joints` (contacts et géométrie), placement et fabrication.
L'UI ne calcule aucune découpe ; elle modifie le document, enregistre l'historique
et affiche les résultats du même moteur utilisé par la CLI. Voir l'[ADR 0004](adr/0004-furniture-composer.md).

## Frontière des identifiants Rhai

L’API `fablabcam_studio::script` est commune au Studio et à la CLI. Les entiers
Rhai sont signés sur 64 bits ; les références de formes du projet sont des `u32`.
Une conversion unique et vérifiée (`u32::try_from`) refuse les valeurs hors plage
avant l’appel métier, aussi pour chaque élément d’un tableau. Une conversion
avec `as` pourrait tronquer un identifiant et viser une autre forme. La plage
numérique `0..=4294967295` ne garantit pas l’existence de la référence : ce
contrôle reste dans `Project`.

Le moteur exécute le script sur une copie. L’éditeur ne remplace le projet et
son historique qu’en cas de succès ; la CLI n’écrit la sortie qu’après réussite.
Les tests de l’issue #19 vérifient les erreurs de conversion, les historiques
Annuler/Rétablir et la préservation des fichiers, y compris une sortie existante
ou égale à l’entrée. Ce correctif ne change ni les formats ni l’allocation des
identifiants du projet.
