# Profils de machine et G-code (v0.3.0)

Implémente l'[issue #9](https://github.com/FabLabCAM/core/issues/9).
Le [guide utilisateur](https://fablabcam.github.io/docs/guide/machines/) détaille
les commandes et les exemples. La [décision d'architecture](adr/0001-machine-gcode.md)
explique les frontières des modules.

## API et composants

- `fablabcam_cam::gcode::machine::{Profile, parse, validate, convert}` : moteur pur.
- `fablabcam_studio::gcode_store::Store` : CRUD des profils et révisions locales.
- `fablabcam_cli::machine::run` : sortie JSON et code de retour ; la commande
  historique `fablabcam gcode` continue à générer les parcours CAO/FAO.
- `profiles/mach3-mill.json` : référence non configurée ;
  `profiles/mach3-mill-demo.json` : machine fictive utilisée par les tests.

```rust
use fablabcam_cam::gcode::machine::{Profile, validate};
let profile = Profile::from_json(&std::fs::read_to_string("machine.json")?)?;
let report = validate(&std::fs::read_to_string("program.nc")?, &profile);
println!("{:?}", report.status);
```

Le schéma JSON 1 refuse les propriétés inconnues. `semantics` choisit explicitement
le modèle `mach3_mill` ou `rs274_mill` ; le nom commercial du contrôleur ne
détermine pas le sens des codes. G70/G71 ne sont interprétés comme unités que
dans le modèle Mach3 Mill. Ces modèles ne couvrent pas les cycles de tournage. Distances, origines et positions
sont en mm, vitesses linéaires en mm/min, broche en tr/min. Les métadonnées
rotatives utilisent degrés et degrés/min ; la cinématique rotative n'est pas
interprétée. `initial_position_mm` et les bornes sont dans le repère machine ;
les origines G54–G59 donnent le décalage du repère pièce par rapport à ce repère.

## Contrat du rapport

`valid` signifie conforme aux paramètres configurés dans le sous-ensemble analysé.
`invalid` contient au moins une erreur. `incomplete` signifie qu'une vérification,
un état ou une durée manque ; le CLI utilise respectivement les codes 0, 1 et 2.
`limits_checked` renseigne la présence des limites XYZ, avances et broche.
`statistics.complete` indique la complétude du modèle statique, pas une mesure
machine. Les diagnostics contiennent sévérité, identifiant, ligne, colonne
(en caractères, base 1), mot concerné et explication.

Mouvements modélisés : G0/G1 et G2/G3 dans G17/G18/G19, centres IJK relatifs ou
absolus, rayon R signé et hélices. Les extrema des arcs participent aux bornes.
G20/G21 et les alias Mach3 G70/G71, G90/G91, G93/G94/G95, origines configurées
G54–G59 et déplacements linéaires G53 sont pris en charge. Une modification
unités/mode d'avance exige un nouveau F avant la prochaine avance.

Les durées sont des estimations à vitesse constante, avec `rapid_mm_min` pour
G0. Elles excluent accélération, jerk, latences et temps d'outil/opérateur. Les
arcs utilisent une borne conservatrice pour la vitesse de chacun des axes du
plan ; elle peut refuser un arc court dont le maximum théorique n'est pas atteint.

## Limites explicites

- Cycles, transformations, compensation active, palpage, référencement,
  sous-programmes, paramètres et macros : reconnus lorsqu'autorisés, mais non
  exécutés. Les statistiques s'arrêtent au premier état de mouvement inconnu.
- G4 : unités de temporisation configurables dans le contrôleur ; durée partielle.
- M6 : peut déplacer les axes. `m6_preserves_position` ne doit être vrai qu'après
  vérification de la configuration locale. M0/M1 rendent la durée partielle.
- Table d'outils, park, position de changement, accélération et convention des
  axes sont conservés comme métadonnées ; ils ne créent aucun mouvement implicite.
- Pas de simulation de collision, correction d'outil ou pilotage machine.
- Les rapports invalides/incomplets peuvent être archivés ; la conversion les refuse.

La conversion normalise les mots, adapte les commentaires et les numéros N,
explicite les modes initiaux et remplace G70/G71 par G20/G21. Elle conserve les
valeurs exactement : aucune baisse de précision, translation de repère, expansion
de cycle, réécriture de macro ou conversion de message MSG. Origines et position
initiale doivent correspondre. La cible est validée avant toute écriture ; les
obstacles figurent dans `unsupported`. Aucun fichier existant n'est écrasé.

## Référence Mach3 Mill

Source primaire : [Using Mach3Mill 1.84-A2, chapitre 10](https://www.machsupport.com/wp-content/uploads/2013/02/Mach3Mill_1.84.pdf).
Les listes viennent des sections 10.7 et 10.8 ; G90.1/G91.1 sont décrits en
10.7.26. Ce profil n'est pas une certification de Mach3 R3.043.066 ou de Mach3Turn.
Les G74/G76 et G10 L20 proposés dans l'issue ne sont pas validés pour ce profil
Mill. Les commentaires documentés sont `(...)`, `//` et les lignes commençant
par `%` ; le `;` n'est pas activé sans vérification locale. Les M-codes utilisateur
ne sont jamais autorisés implicitement. Les variantes G59.x peuvent être ajoutées
localement à la liste blanche, mais leur sémantique reste non interprétée.

## Archives et tests

`Store` verrouille les écritures avec `std::fs::File::lock` (Rust 1.89+). Chaque
révision contient `program.nc`, `profile.json`, `report.json`, `revision.json`.
Le manifeste enregistre l'auteur, la date Unix UTC, le statut et les SHA-256.
Les écritures passent par un répertoire temporaire puis un renommage ; les
révisions inachevées `.pending-*` sont ignorées et peuvent être retirées après
arrêt des écrivains. Les lectures contrôlent les trois empreintes ; ce mécanisme
détecte une corruption et n'est pas une signature contre un attaquant pouvant
réécrire le magasin. Ne pas modifier les révisions à la main.

```sh
cargo test --locked -p fablabcam-cam --test machine --test machine_arcs
cargo test --locked -p fablabcam-studio --test gcode_store
cargo test --locked -p fablabcam-cli --test machine_cli
```

Les tests couvrent géométrie des arcs, positions, unités, vitesses, syntaxe,
conversion sans perte, sorties CLI, archives immuables, intégrité et concurrence.
Les fixtures sont synthétiques et ne qualifient aucune machine physique.
