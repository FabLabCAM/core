# Issue #23 — diagnostic du lancement dans Mach3

## État du diagnostic

Signalement : les quatre exports KO se chargent dans Mach3 mais ne démarrent pas,
alors que les trois exports de référence sont déclarés fonctionnels.
[Originaux et contexte](https://github.com/FabLabCAM/core/issues/23).

**Les photos IMG_5799/5800/5801 identifient maintenant un arc dégénéré** dans
`projet_test002` : Mach3 affiche « Arc de rayon nul » et « Le rayon de fin d'arc
diffère de celui de début d'arc », à la ligne affichée 7393. Le bloc est la ligne
physique 7394 (base 1) de l'original :

```gcode
G1 X85.866 Y36.55
G2 X85.866 Y36.549 I0 J-0.001
```

En IJ incrémental, le centre est (85,866 ; 36,549), exactement l'arrivée.
Le rayon initial vaut 0,001 mm et le rayon final zéro. Le défaut d'export est
reproduit à partir d'un demi-cercle valide de rayon 0,0007 mm, arrondi à trois
décimales. Le préambule seul ne répare pas cette géométrie.

**Correction du diagnostic initial :** le contrôle vérifiait le rayon initial
et l'écart absolu des rayons, mais pas la nullité du rayon final. Un écart de
0,001 mm passe sous 0,002 mm tout en décrivant un arc impossible. L'affirmation
initiale « aucun rayon nul » était donc incorrecte. Le validateur rejette
maintenant un rayon nul à l’une des deux extrémités, indépendamment de cette
tolérance.

Les photos expliquent le refus sur ce bloc ; aucun essai du correctif dans Mach3
ou sur CNC n'a été effectué ici. `lyon3.v1` ne contient pas cette dégénérescence :
son blocage reste à diagnostiquer séparément. Ne pas fermer l'issue avant les
retours sur les fichiers corrigés.

## Mesures sur les fichiers originaux

Lecture statique en G21/G90/G17, avec centres IJ incrémentaux :

| Fichier | Lignes | Arcs | Écart maximal des rayons (mm) | Rayons finaux nuls | Fins de ligne |
|---|---:|---:|---:|---:|---|
| Référence Profile1 | 847 | 613 | 0,000104 | 0 | CRLF |
| Référence Profile2 | 847 | 613 | 0,000104 | 0 | CRLF |
| Référence Profile3 | 847 | 613 | 0,000104 | 0 | CRLF |
| lyon3.v1.KO.nc | 4 258 | 1 072 | 0,000996 | 0 | LF |
| lyon3.v2.KO.nc | 18 606 | 6 840 | 0,001357 | 38 | LF |
| projet_test001.KO.nc | 18 606 | 6 840 | 0,001357 | 38 | LF |
| projet_test002.KO.nc | 18 606 | 6 840 | 0,001357 | 38 | LF |

Les maxima sont arrondis vers le haut. Aucun rayon **initial** nul, aucun arc
aux extrémités XY identiques et aucun écart supérieur à 0,002 mm. Mais trois KO
contiennent chacun 38 rayons **finaux** nuls, tous avec un rayon initial de
0,001 mm ; les trois références et `lyon3.v1` n’en contiennent aucun. Les KO sont
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
Les photos identifient ici une erreur d'arc loin de M6 ; cette hypothèse initiale
n'explique pas le bloc signalé. Ne pas supprimer M6 ou changer le numéro d'outil pour masquer
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
Ce préambule ne change ni les avances, ni la broche, ni les outils, ni les macros
M6, ni les origines. Les photos prouvent qu'il doit être complété par une
correction des arcs après arrondi.

La couverture en pouces a également révélé que le validateur appliquait 0,002 mm
aux deux unités. Il utilise maintenant la tolérance documentée de l'unité active,
convertie en mm pour le calcul. Les arcs au-delà de cette limite restent refusés.

La suite complète a ensuite révélé une troncature `g as i32` dans le backplot :
G91.1 activait G91 et déformait aperçu/simulation. Le parseur distingue désormais
les modes de centre des modes de position, gère les centres absolus dans les
trois plans et signale les codes fractionnaires inconnus sans les tronquer.
Quatre tests ont échoué avant cette correction et passent après ; les tests
d'export comparent aussi les bornes du backplot et de l'analyse machine.

## Arcs après arrondi : correction commune aux exports

Les émetteurs intégré et CBPP évaluent les coordonnées réellement formatées,
y compris le départ et les centres absolus/incrémentaux. Si un rayon s'annule,
ou si un arc partiel devient un cercle complet par arrondi des extrémités,
l'arc est subdivisé en G1 depuis la géométrie non arrondie. Les vrais cercles
représentables restent en G2/G3. Le même découpage que l'option CBPP de conversion
en lignes est partagé dans `gcode/arc_output.rs` ; les arcs majeurs, le sens,
les Z intermédiaires d'hélice, l'arrivée et l'avance sont préservés.

La tolérance de corde automatique est la moitié du pas décimal de sortie
(0,0005 mm à trois décimales), bornée également par `ArcToLinesTolerance` pour
le CBPP. L'arrondi final ajoute au plus un demi-pas par coordonnée. Cette règle
suit les unités du trajet, y compris un trajet déjà converti en pouces.
Les gabarits personnalisés restent responsables des macros qui changent les
coordonnées ou les modes. La subdivision conserve les bornes historiques de
l'option CBPP (tolérance minimale 1e-9, au plus 100 000 segments par arc).

Les copies `ARCS_A_TESTER` des anciens KO remplacent seulement les 38 micro-arcs
dégénérés par G1 à la même arrivée et complètent le préambule. Les NC déjà
arrondis ne permettent pas de retrouver les arcs originaux : ces copies de
diagnostic sont distinctes d'un export recalculé. Un manifeste liste les lignes
modifiées et les empreintes ; aucun original n'est modifié. `lyon3.v1` reçoit
seulement le préambule. Les photos personnelles ne sont pas ajoutées aux sources.

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
cargo test --locked -p fablabcam-cam --test machine_arcs --test arc_rounding
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

Résultats locaux du 4 novembre 2026 (macOS Apple Silicon) : **402 tests réussis**,
Clippy workspace/all-targets avec `-D warnings` réussi. Les dix tests précédents restent couverts. Cinq tests supplémentaires couvrent
le bloc exact, les deux sens, les centres absolus/incrémentaux, trois/quatre
décimales, les cercles conservés et les arcs majeurs/hélicoïdaux. Les deux
régressions d’export ont échoué avant correction. Construction CLI, exemple public
`plaque.rhai` → CB → NC avec le gabarit de diagnostic → SVG vérifiés. Guide :
MkDocs strict, 36 pages sans lien local manquant et rendu du diagnostic inspecté.

Le validateur corrigé s’arrête au premier arc dégénéré de chacun des trois KO
lorsque le préambule est explicité pour permettre leur interprétation : ligne
physique 7394 pour les deux `projet_test`, 9818 pour `lyon3.v2`. Le calcul
géométrique séparé dénombre les 38 occurrences de chaque fichier. Il ne
signale plus d'erreur `arc` ou `preamble` dans les quatre copies `ARCS_A_TESTER`
ni dans le réexport du DXF (toujours 33 481 lignes). **Les rapports restent
incomplets** : courses, broche et vitesses réelles ne sont pas configurées.
Les références sont comparées par calcul géométrique séparé ; leurs préambules
ne satisfont pas toutes les exigences du profil de diagnostic. Aucun de ces
résultats n'est une qualification machine.

## Validation restante

Recharger `projet_test002.ARCS_A_TESTER.nc` dans Mach3 et vérifier la disparition
de l'erreur d'arc pendant la lecture, sans lancer l'usinage. Consigner le résultat
pour les trois fichiers concernés. Obtenir séparément le message/la ligne de
`lyon3.v1` s'il reste refusé. Les copies précédentes `MODES_A_TESTER` sont
insuffisantes pour corriger le bloc photographié. La validation des paramètres
et de l'usinage reste distincte ; aucune nouvelle version n'est publiée ici.
