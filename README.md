# FabLabCAM

CAO/FAO libre en Rust pour les FabLabs : dessin 2D, préparation d'usinage 2,5D,
G-code, visualisation des trajectoires, simulation de matière et meubles paramétriques.

Dépôt : https://github.com/FabLabCAM/core · Licence : [Apache-2.0](LICENSE).

[Documentation du produit](https://fablabcam.github.io/docs/) ·
[Téléchargements](https://github.com/FabLabCAM/core/releases)

## Architecture

| Crate | Responsabilité |
|---|---|
| `fablabcam-geom` | Polylignes, arcs, décalages et opérations géométriques |
| `fablabcam-model` | Documents, calques, entités et format XML `.cb` |
| `fablabcam-cam` | Profils, poches avec îlots, perçage et post-processeurs |
| `fablabcam-backplot` | Lecture G-code, scène 3D et exports SVG/HTML |
| `fablabcam-studio` | Édition, outils, opérations, simulation et scripts Rhai |
| `fablabcam-cli` | Exécutable `fablabcam` |
| `fablabcam-view` | Exécutables `fablabcam-view` et `fablabcam-studio-ui` (egui) |

Les moteurs de géométrie et d'usinage restent indépendants de l'interface.
Voir [les notes d'architecture](docs/architecture.md).

## Construire et essayer

Installer Rust 1.89 ou supérieur avec Cargo, puis :

```sh
git clone https://github.com/FabLabCAM/core.git
cd core
cargo build --locked --workspace
cargo run --locked -p fablabcam-cli --bin fablabcam -- help
cargo run --locked -p fablabcam-view --bin fablabcam-studio-ui -- --demo
```

Sous Ubuntu, le viewer nécessite notamment `pkg-config`, `libx11-dev`,
`libxcursor-dev`, `libxi-dev`, `libxrandr-dev`, `libxkbcommon-dev`, `libwayland-dev`
et `libgl1-mesa-dev`. La CI décrit l'installation utilisée pour les tests.

Exemples de ligne de commande :

```sh
cargo run --locked -p fablabcam-cli -- info crates/fablabcam-cli/tests/fixtures/plaque.cb
cargo run --locked -p fablabcam-cli -- styles tests/fixtures/styles-demo.xml
cargo run --locked -p fablabcam-cli -- gcode crates/fablabcam-cli/tests/fixtures/plaque.cb --op profile-outside --layer Contour --tool 6 --depth -3 --step 1 -o plaque.nc
cargo run --locked -p fablabcam-cli -- plot plaque.nc -o trace.html
cargo run --locked -p fablabcam-cli -- web demo -o studio.html
```

Les valeurs des fixtures servent aux tests ; adapter les réglages à la machine,
au matériau et à l'outil avant tout usinage réel.

## Meubles pour le FabLab (0.5.0)

Ouvrez **Meubles** dans le Studio : créez un modèle de caisson ou d'étagère,
utilisez-le pour un meuble, puis réglez matière, CNC et dalles. L'assistant produit
un dossier ZIP avec plans CB/DXF/SVG, liste de débit CSV et notice PDF.
Les modèles et meubles s'échangent au format `.fcm-template.json` / `.fcm.json`.

```sh
fablabcam furniture template cabinet -o caisson.fcm-template.json
fablabcam furniture new caisson.fcm-template.json -o caisson.fcm.json
fablabcam furniture plan caisson.fcm.json
fablabcam furniture pack caisson.fcm.json -o fabrication.zip
```

Voir le [guide illustré](https://fablabcam.github.io/docs/guide/meubles/) et les
[formats, calculs et limites](docs/furniture.md). Les opérations FAO restent à
préparer. Une dalle associée à un profil incomplet ne peut pas exporter de G-code.

## Profils de machine (depuis 0.4.1)

Dans le Studio, ouvrez **Profils machine** ou **Outils → Profils machine · Atelier**.
Créez un profil avec l'assistant, modifiez ses paramètres, dupliquez-le ou
importez/exportez son JSON. Le [guide Atelier](https://fablabcam.github.io/docs/guide/profils-studio/)
illustre le parcours et les confirmations. Les profils sont indépendants du dessin.

La commande `fablabcam machine` analyse, valide, convertit et archive les G-code
avec un profil JSON. Voir le [guide technique](docs/machine-gcode.md),
les [notes de version](CHANGELOG.md) et le [guide utilisateur](https://fablabcam.github.io/docs/guide/machines/).

```sh
cargo run --locked -p fablabcam-cli -- machine validate crates/fablabcam-cam/tests/fixtures/machine-valid.nc --profile profiles/mach3-mill-demo.json
```

Le profil de démonstration décrit une machine fictive ; le profil de référence
ID CNC PRO - Mach3 est le modèle par défaut, à configurer pour votre atelier.
Le fichier `profiles/id-cnc-pro.json` contient les sources et les indications
nominales ; les réglages de la machine restent à confirmer.

## Tests

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

La CI exécute ces vérifications sous Linux, Windows et macOS. Le fichier
`Cargo.lock` est conservé pour construire avec les versions de dépendances testées.

## Formats et distribution

- Dessins : `.cb` (XML), DXF et SVG ; programmes : G-code.
- Modèles et meubles : JSON natif versionné ; dossiers de fabrication : ZIP.
- Le format `.cb` du Studio ajoute des outils et opérations propres à FabLabCAM.
  Voir [les limites d'interopérabilité](docs/file-formats.md).
- [Construction des applications](docs/building.md) pour macOS, Windows et Linux.
- [Contribuer](CONTRIBUTING.md) et [signaler une vulnérabilité](SECURITY.md).

Le code du projet est distribué sous Apache-2.0. Les dépendances conservent leurs
licences ; voir [NOTICE](NOTICE). Les données de test synthétiques sont incluses
sous la licence du projet. Aucune bibliothèque propriétaire n'est distribuée.
