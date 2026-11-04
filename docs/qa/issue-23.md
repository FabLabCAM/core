# Issue #23 — diagnostic du lancement dans Mach3

## État du diagnostic

Signalement : les quatre exports KO se chargent dans Mach3 mais ne démarrent pas,
alors que les trois exports de référence sont déclarés fonctionnels.
[Originaux et contexte](https://github.com/FabLabCAM/core/issues/23).

**La cause exacte sur l'installation Mach3 n'est pas encore confirmée.** La barre
d'état, la ligne d'arrêt, la version, le mode IJ et les macros M6 de cette machine
ne sont pas disponibles. Aucun essai de Mach3 ou de CNC n'a été effectué ici.
Ne pas fermer l'issue sur la seule base des tests logiciels.

## Mesures sur les fichiers originaux

Lecture statique en G21/G90/G17, avec centres IJ incrémentaux :

| Fichier | Lignes | Arcs | Écart maximal des rayons (mm) | Fins de ligne |
|---|---:|---:|---:|---|
| Référence Profile1 | 847 | 613 | 0,000104 | CRLF |
| Référence Profile2 | 847 | 613 | 0,000104 | CRLF |
| Référence Profile3 | 847 | 613 | 0,000104 | CRLF |
| lyon3.v1.KO.nc | 4 258 | 1 072 | 0,000996 | LF |
| lyon3.v2.KO.nc | 18 606 | 6 840 | 0,001357 | LF |
| projet_test001.KO.nc | 18 606 | 6 840 | 0,001357 | LF |
| projet_test002.KO.nc | 18 606 | 6 840 | 0,001357 | LF |

Les maxima sont arrondis vers le haut. Aucun rayon nul, aucun arc aux extrémités
XY identiques et aucun écart supérieur à 0,002 mm dans ces fichiers. Les KO sont
ASCII, les lignes les plus longues font 37 ou 39 caractères. Ils terminent tous
par `M30` et un LF. La longueur du fichier ne prouve pas une erreur géométrique.

Le [manuel officiel Mach3 Mill 1.84-A2](https://www.machsupport.com/wp-content/uploads/2013/02/Mach3Mill_1.84.pdf)
précise :

- §10.5 : CR, LF ou CRLF sont admis ; conserver LF ne constitue pas ici un défaut.
- §10.7.3.2 : écart admissible de 0,002 mm en G21, ou 0,0002 pouce en G20.
- §10.7.26 : G90/G91 et G90.1/G91.1 sont des réglages distincts.
- §10.8.3 : M6 peut appeler M6Start, attendre Cycle Start puis appeler M6End.

L'ordre `M6 T1` est légal : l'ordre d'exécution d'un bloc sélectionne l'outil
avant le changement (§10.12). Les KO sélectionnent T1, les références T0.
Ce changement de numéro peut solliciter une macro différente ; il reste à vérifier
sur l'installation. Ne pas supprimer M6 ou changer le numéro d'outil pour masquer
un arrêt attendu.

## Défaut corrigé dans l'exporteur

L'export intégré établissait seulement `G21 G90 G17 G94` (G20 en pouces).
Il calculait des centres IJ incrémentaux sans les déclarer et conservait
d'éventuelles compensations actives. Le profil Mach3 fourni exige pourtant
G40/G49/G80/G91.1 avant les premiers mouvements. Les références annulent G40,
mais elles aussi dépendent d'un mode IJ initial non déclaré.

Le préambule intégré établit désormais :

```gcode
G21 G90 G17 G94 G40 G49 G80 G91.1
```

G20 remplace G21 pour un export en pouces. Le gabarit CBPP par défaut établit
les mêmes modes, mais `{$arccentermode}` suit la convention configurée :
G90.1 pour des centres absolus. Un Header CBPP explicite reste prioritaire.
Les trajectoires, avances, broche, numéro d'outil, macros M6 et origines de travail
ne sont pas modifiés. Il s'agit d'un défaut reproduit, pas d'une preuve suffisante
qu'il explique à lui seul le blocage signalé.

La couverture en pouces a également révélé que le validateur appliquait 0,002 mm
aux deux unités. Il utilise maintenant la tolérance documentée de l'unité active,
convertie en mm pour le calcul. Les arcs au-delà de cette limite restent refusés.

La suite complète a ensuite révélé une troncature `g as i32` dans le backplot :
G91.1 activait G91 et déformait aperçu/simulation. Le parseur distingue désormais
les modes de centre des modes de position, gère les centres absolus dans les
trois plans et signale les codes fractionnaires inconnus sans les tronquer.
Quatre tests ont échoué avant cette correction et passent après ; les tests
d'export comparent aussi les bornes du backplot et de l'analyse machine.

## Reproduction depuis les sources fournies

Le DXF s'importe en 20 formes (HATCH et VIEWPORT sont signalés comme ignorés).
Le profil extérieur de toutes ces formes faisait échouer deux assertions de
`cavalier_contours` en debug : positions répétées, puis intersection sur des
segments redondants. Les normalisations `remove_repeat_pos` puis
`remove_redundant` fournies par la bibliothèque sont maintenant appliquées
avant les offsets, à sa propre tolérance,
sur une copie. Deux régressions vérifient contours ouverts/fermés, aires,
périmètres, bulges et conservation du dessin original.

Après correction, la CLI produit 33 481 lignes pour toutes les formes, outil
3 mm, profondeur −3,6 mm, passes 3 mm, retrait 3 mm, avances 1000/500 et broche
6000. Ce programme n'est pas présenté comme équivalent aux références : la
sélection et les transformations des projets KO ne sont pas connues.

Le `.cb` de référence contient des splines non prises en charge par l'import
XML actuel. Son réexport direct ne contient que 33 lignes et ne couvre pas les
contours de la référence. Il n'est pas livré comme fichier corrigé. Cette limite
d'interopérabilité est distincte du lancement des G-code déjà exportés.

## Vérifications reproductibles

```sh
cargo test --locked -p fablabcam-studio --test mach3_export
cargo test --locked -p fablabcam-cam --test machine_arcs
cargo test --locked -p fablabcam-backplot --test arc_modes
cargo test --locked -p fablabcam-geom --test repeated_vertices
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Trois tests d'export ont échoué avant le correctif. Ils couvrent l'émetteur CAM,
le Studio en mm/pouces, le CBPP par défaut et les centres absolus, avec un état
initial volontairement incompatible (G20/G91/G18/G93 et mode IJ opposé).
Un quatrième test reproduit le faux rejet de 0,0001 pouce d'écart de rayon et
vérifie que 0,00254 mm en G21 et 0,0003 pouce en G20 restent refusés.
Le profil synthétique suppose explicitement que M6 préserve l'état ; cette
hypothèse de test ne qualifie aucune macro réelle.

Résultats locaux du 4 novembre 2026 (macOS Apple Silicon) : **397 tests réussis**,
Clippy workspace/all-targets avec `-D warnings` réussi. Dix nouveaux tests
ont reproduit leurs défauts avant correction. Construction CLI, exemple public
`plaque.rhai` → CB → NC avec le gabarit de diagnostic → SVG vérifiés. Guide :
MkDocs strict, 36 pages sans lien local manquant et rendu du diagnostic inspecté.

Les quatre copies des KO dont seul le préambule a été complété n'ont plus de
diagnostic `preamble` ou `arc` avec le profil de diagnostic (origine fictive
connue et M6 supposé préserver les modes). Le programme régénéré depuis le DXF
donne le même constat. **Les rapports restent incomplets** : courses, broche et
vitesses réelles ne sont pas configurées. Aucun de ces résultats n'est une
qualification machine ni une confirmation de démarrage dans Mach3.

## Validation restante

Relever le message et la ligne affichés après Cycle Start avec un KO original,
puis avec une copie ne changeant que le préambule. Conserver les réglages machine
et le reste du programme identiques pour isoler l'effet des modes. Distinguer
un arrêt sur M6 d'une erreur de lecture ou d'arc. Consigner le résultat dans #23
avant de déclarer le problème résolu ou de publier une correction comme validée
sur cette installation.
