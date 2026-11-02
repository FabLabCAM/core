# Construction et distribution

Depuis la racine du dépôt, avec Rust stable :

```sh
cargo build --locked --release --workspace
```

## macOS

```sh
docs/build-app.sh
open "dist/FabLabCAM Studio.app"
```

Le bundle utilise une signature ad hoc et n'est pas notarié par Apple.

## Compilation croisée depuis macOS

`docs/build-cross.sh windows linux` construit les exécutables x86_64. Installer
les cibles Rust `x86_64-pc-windows-gnu` et `x86_64-unknown-linux-gnu`, puis MinGW-w64
et Zig. `docs/build-appimage.sh` ajoute un AppImage Linux ; il nécessite squashfs
et utilise les outils d'image macOS. Ces scripts ne constituent pas des tests
fonctionnels de l'interface sur ces systèmes.

Les paquets incluent LICENSE et NOTICE. Avant toute publication de binaires,
compléter les notices des dépendances embarquées selon leurs licences.

Les démonstrations web se régénèrent avec `fablabcam web demo -o studio.html` et
`fablabcam plot programme.nc -o trace.html`. Les captures historiques ne font pas
partie du dépôt public.
