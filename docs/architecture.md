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
