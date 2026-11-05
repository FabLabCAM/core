# Notes de version

## 0.7.1 — 2026-11-05

Cette version reprend également les corrections Mach3 de 0.7.1-rc.1 décrites
ci-dessous. Les essais des trois autres fichiers de #23 restent attendus ;
la publication ne vaut pas qualification d’une machine.

### Import SVG — [#26](https://github.com/FabLabCAM/core/issues/26)

- Composition des transformations SVG imbriquées et locales avant inversion Y ;
  conservation des arcs circulaires sous similitude et approximation sous matrice
  non uniforme.
- Exclusion des définitions et ressources inutilisées du dessin importé.
- Lecture des exposants et commandes implicites via `svgtypes`, avec erreur
  explicite pour les chemins ou transformations invalides.
- Régressions du moteur et de la CLI, conservation des arcs de fermeture,
  documentation du périmètre et vérification locale du fichier signalé.

## 0.7.1-rc.1 — 2026-11-05

Préversion destinée à la validation Mach3 de #23. Le test utilisateur confirme
`lyon3.v1` ; les trois autres fichiers restent à vérifier.

### Arcs et initialisation du G-code — [#23](https://github.com/FabLabCAM/core/issues/23)

- Arcs devenant dégénérés après arrondi : subdivision en G1 selon la précision
  de sortie, commune aux exports intégré et CBPP, avec conservation des cercles
  représentables et des hélices. Corrige le bloc identifié par les photos Mach3.
- Validateur : refus d'un rayon final nul même si l'écart entre les rayons est
  inférieur à la tolérance ; régression du bloc exact affiché ligne 7393.
- Exports intégrés : annulation des compensations/cycles et déclaration explicite
  des centres IJ incrémentaux avant mouvement. Le CBPP par défaut déclare la
  convention de centre configurée ; les en-têtes personnalisés restent prioritaires.
- Analyse d'arcs : tolérance de rayon adaptée à l'unité active, conformément au
  manuel Mach3 (0,002 mm ou 0,0002 pouce).
- Backplot/simulation : G90.1/G91.1 ne sont plus tronqués en G90/G91 ; les centres
  absolus sont relus dans les trois plans et les codes fractionnaires inconnus
  sont signalés au lieu d'être interprétés comme une autre commande.
- Calcul des décalages : normalisation des sommets répétés et redondants avant
  l'appel au noyau géométrique, sur une copie du contour ; évite les assertions
  rencontrées lors de la reproduction depuis le DXF de #23.
- Régressions sur les modes initiaux, mm/pouces et centres absolus ;
  [diagnostic](docs/qa/issue-23.md). Bloc dégénéré identifié dans trois KO ;
  retour Mach3 du correctif encore à confirmer. Pour `lyon3.v1`, le refus du
  premier arc en IJ absolu hérité est reproduit ; G91.1 corrige cette dépendance
  dans les trois exports, désormais couverts par une régression dédiée.
- Retour utilisateur du 4 novembre 2026 : la copie de `lyon3.v1` ajoutant seulement
  G91.1 fonctionne dans Mach3. Confirmation des trois autres KO encore attendue.

## 0.7.0 — 2026-11-04

### Assistant natif — [#20](https://github.com/FabLabCAM/core/issues/20)

- Panneau commun aux pages CAO, 3D, Simulation, Scripts, Outils, Profils et Meubles ;
  conversation Markdown, contexte filtrable/épinglable et historique local.
- Modes Ask, Plan et Agent ; propositions éditables, aperçu calculé sur copie,
  application explicite de chaque lot, refus des états obsolètes, annuler/rétablir.
- Actions dessin, Rhai, meubles/composition/assemblages, outillage, opérations
  et préparation d’exports par dialogue natif. Aucun pilotage de CNC.
- OpenAI Responses, Claude Messages, Mistral et Ollama : streaming, test des outils,
  annulation, délais, erreurs classées et clés séparées dans le coffre système.
- Exécution Rhai hors rendu, limites et imports désactivés ; export ZIP hors rendu.
- Tests déterministes des quatre protocoles et essais réels Mistral Small.
  OpenAI, Claude et Ollama restent à qualifier avec leurs services réels ;
  l’issue #20 reste ouverte pour ce suivi.
- Guide illustré, architecture, mémoire et rapport de validation actualisés.

## 0.6.1 — 2026-11-04

### Identifiants de formes dans les scripts — [#19](https://github.com/FabLabCAM/core/issues/19)

- Les scripts Rhai refusent les identifiants négatifs ou supérieurs à `u32::MAX`,
  seuls comme dans un tableau, avant toute conversion. Une valeur hors plage
  ne peut plus désigner une autre forme par troncature.
- Le diagnostic conserve la valeur fautive et la position dans le script. Les
  identifiants valides et les erreurs de forme inexistante restent inchangés.
- Tests de régression : dix fonctions, tableaux mixtes, bornes, types invalides,
  projet et historique préservés ; code de retour CLI et fichiers inchangés en échec.
- Mémoire, architecture, guide Rhai et suivi des versions actualisés.

## 0.6.0 — 2026-11-04

### Composer des meubles et choisir les assemblages — [#17](https://github.com/FabLabCAM/core/issues/17)

- Éditeur visuel : cases, tablettes, séparations, grille, positions fixes/égales,
  épaisseurs individuelles, bacs ouverts, copie/répétition et annuler/rétablir.
- Modèles et projets conservent toute la composition. Schéma 2, lecture du schéma 1,
  migration explicite et sauvegarde du catalogue précédent.
- Assemblage par défaut et surcharges par liaison, orientation des angles,
  jeu total, variantes traversantes/borgnes, dogbone/T-bone et pièces complémentaires.
  Huit familles orthogonales disponibles ; autres familles visibles et désactivées.
- Plans réels CB/DXF/SVG, poches/profondeurs, manifestes et montages recto/verso.
  Placement par épaisseur, enveloppes incluant tenons et dégagements.
- Contrôle FAO des opérations nécessaires ; refus des poches absentes ou devenues
  îlots, maintien de la validation distincte du G-code et du profil machine.
- Guide, formats, architecture et mémoire de maintenance actualisés. La tenue
  mécanique, les coulisses, la quincaillerie et l'édition 3D libre ne sont pas calculées.

## 0.5.0 — 2026-11-04

### Meubles paramétriques — [#14](https://github.com/FabLabCAM/core/issues/14)

- Nouvel espace **Meubles** : bibliothèques de modèles réutilisables et de projets,
  création, modification, duplication, annulation/rétablissement et import/export
  natif JSON versionné avec aperçu et copie en cas de collision.
- Assistant **Meuble → Matière & CNC → Découpe → Dossier**, caisson et étagère
  à panneaux vissés, dimensions extérieures, épaisseur, dos et tablettes.
- Aperçus assemblé/éclaté, repères stables, nomenclature et placement rectangulaire
  déterministe avec marges, espacement, rotations et refus des pièces hors dalle.
- Instantanés du modèle et du profil machine ; révisions de modèles indépendantes
  des meubles existants, écritures atomiques et protection des conflits concurrents.
- Dossier ZIP : natif, CB/DXF/SVG par dalle, CSV et notice PDF. Transfert d'une dalle
  vers la FAO ; le profil reste attaché au `.cb`. L'export G-code associé exige
  des opérations calculées et une analyse valide, jamais un rapport incomplet.
- Commandes `fablabcam furniture template|new|validate|plan|pack`, guide illustré,
  exemples reproductibles, décision d'architecture et mémoire de maintenance.

Le caisson 600 × 600 × 800 mm, OSB 20 mm, dos et tablette produit six pièces sur
trois dalles 1 500 × 800 mm (marge 10, fraise 6, espacement 8 mm). Ces dimensions
sont un exemple de travail, pas une qualification des limites d'une CNC.
La V1 ne dimensionne pas la quincaillerie, les ancrages ni les charges. Elle ne
couvre pas les portes, tiroirs, assemblages usinés ou formes arbitraires. Le
placement est heuristique ; la FAO et le choix du post restent explicites.


## 0.4.1 — 2026-11-03

### ID CNC PRO - Mach3 par défaut — [#9](https://github.com/FabLabCAM/core/issues/9)

- Profil `id-cnc-pro-1500x800`, fourni dans `profiles/id-cnc-pro.json`,
  avec identité du LOV, sources et caractéristiques nominales documentées.
- Modèle par défaut de création dans le Studio et `machine profile init`.
  Une bibliothèque vide reçoit ce profil ; les profils existants sont conservés.
- Les paramètres locaux non confirmés restent absents : une course nominale
  ne définit pas les bornes du repère machine. Le modèle reste « À configurer ».
- Exemple JSON, guide, captures du profil par défaut et suivi de l’issue actualisés.

## 0.4.0 — 2026-11-03

### Atelier des profils machine dans le Studio — [#11](https://github.com/FabLabCAM/core/issues/11)

- Espace **Profils machine · Atelier** : bibliothèque locale, édition, duplication,
  import/export JSON et récapitulatif des courses renseignées.
- Création guidée en quatre étapes : identité, axes et sécurité, broche et outils,
  vérification. Les limites inconnues restent absentes ; un profil peut être
  enregistré avec l'état « À configurer » pour être complété plus tard.
- Formulaires G-code et JSON complet pour conserver les paramètres avancés.
  Un JSON invalide n'altère pas les champs et empêche l'enregistrement/export.
- Confirmation des brouillons, import avec copie ou remplacement explicite,
  refus d'écrasement des exports et détection des modifications concurrentes.
- Protection des brouillons lors de la fermeture, y compris « Quitter » sur macOS.
- Guide illustré avec des captures réelles du Studio et suivi de l'issue.

Le schéma JSON reste en version 1 et le magasin est partagé avec la CLI.
Les états de configuration ne valident ni une machine physique ni un programme.
La validation, la conversion et les archives G-code restent accessibles en CLI/API.
Les profils ne sélectionnent pas automatiquement un post-processeur `.cbpp`.

## 0.3.0 — 2026-11-03

### Profils de machine et gestion du G-code — [#9](https://github.com/FabLabCAM/core/issues/9)

- Profils JSON versionnés : contrôleur, axes, courses, avances, broche, dialecte,
  formats, outils et contraintes atelier. Référence Mach3 Mill sourcée et exemple fictif.
- Nouvelle commande `fablabcam machine` : CRUD, analyse, validation, statistiques,
  conversion de formats/dialectes compatibles et archives par révision.
- Diagnostics ligne/colonne et statuts `valid`, `invalid`, `incomplete` ; limites
  XYZ avec extrema d'arcs, unités, modes, avances et broche contrôlés.
- Archives avec instantané du profil/rapport, auteur, date et SHA-256 ; écritures
  concurrentes sérialisées, intégrité vérifiée à la lecture.
- Conversion conservatrice : préserve les valeurs, explicite les modes, adapte
  commentaires/numérotation et alias G70/G71 ; liste les obstacles sans créer de sortie.
- `fablabcam version` / `--version`, profils inclus dans les distributions,
  guide de machine, suivi des issues et procédure de mise à jour documentaire.

La fonctionnalité est accessible en CLI et API Rust. Les cycles, macros,
compensations actives et transformations non interprétés restent explicitement
incomplets. La conversion n'étend pas ces constructions. Le Studio graphique,
le support `.cb` et la licence Apache-2.0 sont conservés. Compilation : Rust 1.89+
(verrouillage des archives), distributions construites avec Rust 1.99.1.

## 0.2.0 — 2026-11-02

Première release publique : Studio 2D/2.5D, outils et opérations, simulation,
scripts Rhai, post-processeurs, CLI et viewer. Installateurs et archives pour
macOS, Windows et Linux, en x64 et ARM64 ; notices tierces et SHA-256.

[Release 0.2.0](https://github.com/FabLabCAM/core/releases/tag/v0.2.0)
