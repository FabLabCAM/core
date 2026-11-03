# Construction et distribution

Depuis la racine du dépôt, avec Rust 1.89 ou supérieur (distributions : 1.99.1) :

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

## Paquets de release

Le workflow `Desktop packages` construit nativement les versions x64 et ARM64
de macOS, Windows et Linux. Il produit des DMG et ZIP macOS, des installateurs
Windows pour l'utilisateur courant et ZIP portables, ainsi que des DEB et
archives tar.gz Linux (glibc 2.35 ou supérieure). Les outils de ligne de commande
et le viewer sont inclus. Les binaires Windows utilisent le runtime C statique.

Les paquets incluent LICENSE, NOTICE, THIRD-PARTY-LICENSES.html, les notices des
polices embarquées et les sources originales des dépendances MPL. Les notices
sont générées avec `cargo-about 0.9.3` et `packaging/notices.py`. Chaque archive
dispose d'une somme SHA-256. Les paquets ne sont pas signés commercialement ;
les applications macOS ont une signature ad hoc, sans notarisation Apple.

Pour reproduire un paquet, installer Python 3.11+ et les outils natifs de son OS,
puis utiliser les commandes et versions du workflow. `packaging/package.py`
assemble les binaires déjà compilés depuis `target/<cible>/release/`. Les
artifacts sont validés avant d'être joints à une release GitHub ; le workflow
de construction n'a aucune permission d'écriture sur le dépôt.

Les démonstrations web se régénèrent avec `fablabcam web demo -o studio.html` et
`fablabcam plot programme.nc -o trace.html`. Les captures historiques ne font pas
partie du dépôt public.

La [procédure de release](releasing.md) relie issues, tests, notes de version et
publication du guide. Les profils JSON sont inclus dans chaque paquet.
