# Modèles, projets et fabrication de meubles

Module initial #14, étendu par #17 en 0.6.0. Voir
[l'ADR de composition](adr/0004-furniture-composer.md) et le
[guide utilisateur](https://fablabcam.github.io/docs/guide/meubles/).

## Formats natifs, schéma 2 (lecture des schémas 1 et 2)

Les deux formats utilisent un objet JSON avec `document_type` et `data` :

```json
{"document_type":"template","data":{"schema_version":2,"id":"…","name":"…","revision":1,"construction":"cabinet","defaults":{},"limits":{}}}
```

Le fragment montre la structure, pas un fichier complet. Les commandes
`fablabcam furniture template cabinet -o exemple.fcm-template.json` et
`fablabcam furniture new exemple.fcm-template.json -o exemple.fcm.json`
produisent des documents complets validés. L'autre gabarit est `shelving`.

- `template` : identité ASCII, révision positive, construction, dimensions par
  défaut (`width`, `depth`, `height`, `thickness`, `back`, `shelves`) et limites.
  `composition` facultative contient arbre de cases, séparations, bacs, épaisseurs,
  continuités, noms et assemblages par défaut/surcharges. Les tablettes anciennes
  ne sont plus utilisées lorsque cette composition existe.
- `furniture` : identité et nom du projet, modèle complet, dimensions propres,
  matériau, profil machine complet et contraintes de placement (`setup`).
- Dimensions en millimètres. Aucun code ou formule arbitraire n'est exécuté.
- Champs inconnus, versions inconnues, limites incohérentes, valeurs non finies
  et fichiers natifs au-delà de 2 Mo sont refusés avant ajout.
- Les résultats dérivés sont recalculés et ne constituent pas des entrées fiables.

Un document de schéma 1 est lu sans réécriture. L'activation de Composer convertit
ses tablettes à géométrie constante, bord à bord. Sauvegarder/exporter écrit le
schéma 2 ; le fichier source reste intact. Au premier enregistrement du catalogue
ancien, `library-schema-1.backup.json` conserve son contenu avant migration.
Une version inconnue est refusée. Les versions 0.5.x ne lisent pas le schéma 2.

## Persistance et historique

`Library` conserve `library.json` dans le dossier de données, sous verrou système.
Chaque sauvegarde compare le document chargé au document courant, puis remplace
atomiquement le catalogue. Un conflit est une erreur, pas un écrasement. Les
nouveaux identifiants combinent date haute résolution, PID et compteur ; une
collision éventuelle est aussi refusée par la sauvegarde.

`Draft` mémorise la référence enregistrée et jusqu'à 128 modifications. Une
saisie de champ est une transaction pour Annuler/Rétablir. Les projets conservent
leur copie du modèle précédent après une nouvelle révision en bibliothèque.

## Calculs et exports

`panels_with_cutter` résout les volumes puis les contours et usinages complémentaires.
`panels` utilise une fraise de 6 mm ; le dossier utilise toujours la fraise du projet.
Les IDs sont stables et chaque paroi partagée appartient à une seule division.
`place` trie les pièces par surface décroissante et découpe les rectangles libres
en bandes disjointes, par épaisseur. Les enveloppes incluent les tenons et les
contours des poches débordant du brut. Les placements sont complets ou refusés ; la surface occupée
ne comprend ni chutes ni marges. Pas de promesse d'optimalité.

Les exports `.cb` réutilisent la géométrie et la persistance existantes. L'extension
optionnelle `<FurnitureMachine>` contient le JSON échappé du profil. Un ancien
fichier sans cet élément garde son comportement ; un profil invalide est refusé
au chargement. Un logiciel tiers peut ignorer cette extension FabLabCAM.

`FurnitureOperations` ajoute les exigences d'usinage (calque, type, profondeur,
côté du profil) aux CB de meubles composés. Le contrôle commun refuse une poche
manquante, une mauvaise profondeur ou une poche multi-contours pouvant conserver
un îlot. Les plans recto/verso et `operations.json` explicitent les montages.
`assemblages.json` conserve la composition et les paires de pièces/faces.

La préparation de dalle n'ajoute aucune opération automatique. Le post-processeur
reste choisi explicitement. `check_cam_export` est commun au Studio et à `build` :
opérations absentes/en erreur et rapports machine invalides/incomplets bloquent
l'export lorsqu'un profil est attaché. Cette analyse n'est pas une qualification
physique. Le CLI `machine validate` reste utilisable pour les rapports détaillés.

Le ZIP contient uniquement les formats choisis et les fichiers de contexte. Les
écritures externes refusent l'écrasement. CSV et JSON préservent les données métier.
La notice PDF utilise Helvetica/WinAnsi pour les libellés fixes, les cotes et les
repères ; les noms libres Unicode sont conservés dans les documents natifs.

## Vérifications

Les tests `furniture_geometry`, `furniture_library`, `furniture_export` et
`furniture_cli` couvrent le cas de référence, des variantes dimensionnelles,
chevauchements, rotations, impossibilités, copies, révisions, imports invalides,
conflits, historique, formats du dossier, extension XML et refus d'export machine.
Les parcours egui et la notice PDF sont également contrôlés visuellement.

Les tests `furniture_composition*`, `furniture_joinery*`, `furniture_composed_export`
et `furniture_cam_safety` couvrent la grille 30 cases, la copie des assemblages,
les épaisseurs, les retournements, les dégagements réellement fraisables, les
poches de réduction, l'aller-retour XML et les protections d'export.
