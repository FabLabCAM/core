# Formats de fichiers

## Projets XML `.cb`

La racine est `CADFile`. Le lecteur reconnaît les calques `layers/layer` et leurs
objets, notamment polylignes, cercles, arcs, rectangles et points. Il accepte
également la variante `Layers/Layer` et les sommets `Point3D`.

L'import des opérations utilise `parts/part/machineops`. Les propriétés non
prises en charge sont signalées. Le Studio enregistre ses outils, brut et
opérations dans les éléments `Tools`, `Stock` et `Operations`. Cette extension
ne garantit pas la relecture par un autre logiciel ni la conservation de toutes
les propriétés d'un fichier importé. L'extension reconnue pour ces projets est `.cb`.

## Styles et post-processeurs

Les styles XML utilisent des éléments `CAMStyle`, un parent `Style` et des valeurs
avec état `Value` ou `Auto`. La fixture originale `tests/fixtures/styles-demo.xml`
couvre sept cas d'héritage et de paramètres imbriqués ; elle n'est pas une
bibliothèque de réglages de production.

Les post-processeurs `.cbpp` sont des gabarits XML avec macros. Les champs inconnus
ou sans effet sont signalés par la CLI.

## Dessins et trajectoires

Le Studio importe et exporte des sous-ensembles DXF et SVG ; les éléments ignorés
sont remontés au client. Le backplot lit notamment G0–G3, G17, G20/G21, G80/G81/G83
et G90/G91. Vérifier le dialecte et le post-processeur de la machine cible.

## Meubles et modèles, schéma 2

Les formats `.fcm.json` et `.fcm-template.json` conservent la composition et les
assemblages ; voir [détail et migration](furniture.md). Les versions 1 et 2 sont
lues, seule la version 2 est écrite lors d'une sauvegarde/export explicite.

Les CB issus des meubles peuvent contenir `FurnitureMachine` (profil machine)
et `FurnitureOperations` (liste JSON échappée des opérations requises). Ces
extensions facultatives sont validées à la lecture ; leur absence préserve les
anciens documents. Un logiciel tiers peut les ignorer. Les plans DXF/SVG restent
2D et doivent être accompagnés du manifeste `operations.json` et des montages
recto/verso ; les profondeurs ne peuvent pas être déduites du seul contour.
