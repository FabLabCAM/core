# Modèles, projets et fabrication de meubles

Implémentation de l'issue #14 pour 0.5.0. Voir
[l'ADR](adr/0003-furniture.md) et le
[guide utilisateur](https://fablabcam.github.io/docs/guide/meubles/).

## Formats natifs, schéma 1

Les deux formats utilisent un objet JSON avec `document_type` et `data` :

```json
{"document_type":"template","data":{"schema_version":1,"id":"…","name":"…","revision":1,"construction":"cabinet","defaults":{},"limits":{}}}
```

Le fragment montre la structure, pas un fichier complet. Les commandes
`fablabcam furniture template cabinet -o exemple.fcm-template.json` et
`fablabcam furniture new exemple.fcm-template.json -o exemple.fcm.json`
produisent des documents complets validés. L'autre gabarit est `shelving`.

- `template` : identité ASCII, révision positive, construction, dimensions par
  défaut (`width`, `depth`, `height`, `thickness`, `back`, `shelves`) et limites.
- `furniture` : identité et nom du projet, modèle complet, dimensions propres,
  matériau, profil machine complet et contraintes de placement (`setup`).
- Dimensions en millimètres. Aucun code ou formule arbitraire n'est exécuté.
- Champs inconnus, versions inconnues, limites incohérentes, valeurs non finies
  et fichiers natifs au-delà de 2 Mo sont refusés avant ajout.
- Les résultats dérivés sont recalculés et ne constituent pas des entrées fiables.

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

`panels` génère les rectangles et les volumes assemblés, avec repères stables.
`place` trie les pièces par surface décroissante et découpe les rectangles libres
en bandes disjointes. Les placements sont complets ou refusés ; la surface occupée
ne comprend ni chutes ni marges. Pas de promesse d'optimalité.

Les exports `.cb` réutilisent la géométrie et la persistance existantes. L'extension
optionnelle `<FurnitureMachine>` contient le JSON échappé du profil. Un ancien
fichier sans cet élément garde son comportement ; un profil invalide est refusé
au chargement. Un logiciel tiers peut ignorer cette extension FabLabCAM.

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
