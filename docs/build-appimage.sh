#!/bin/bash
# Construit « FabLabCAM_Studio-<version>-x86_64.AppImage » pour Linux (Debian, Ubuntu…) depuis macOS.
# usage : docs/build-appimage.sh [dossier-de-sortie]      (défaut : dist/)
#
# Prérequis (une fois) : ceux de docs/build-cross.sh (cible Linux), puis
#   brew install squashfs
# Le runtime officiel AppImage (github.com/AppImage/type2-runtime) est téléchargé dans
# target/appimage/ au premier lancement ; son empreinte SHA-256 est comparée à celle publiée.
#
# Sous Linux : chmod +x FabLabCAM_Studio-*.AppImage puis double-clic ou ./FabLabCAM_Studio-*.AppImage
#   ./FabLabCAM_Studio-*.AppImage projet.cb     ouvre un projet
#   ./FabLabCAM_Studio-*.AppImage cli build …   lance la ligne de commande « fablabcam »
# Il faut fusermount (paquet fuse3 : sudo apt install fuse3), sinon :
#   ./FabLabCAM_Studio-*.AppImage --appimage-extract-and-run
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-dist}"
VERSION="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
VERSION="${VERSION:-0.1.0}"
WORK=target/appimage
RUNTIME="$WORK/runtime-x86_64"
RUNTIME_URL=https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64
RUNTIME_API=https://api.github.com/repos/AppImage/type2-runtime/releases/tags/continuous
ID=org.fablabcam.studio

command -v mksquashfs >/dev/null || { echo "mksquashfs absent : brew install squashfs"; exit 1; }

# exécutables Linux sans symboles de débogage (18 Mo → quelques Mo)
CARGO_PROFILE_RELEASE_STRIP=symbols docs/build-cross.sh linux

mkdir -p "$WORK"
if [ ! -f "$RUNTIME" ]; then
  curl -fsSL -o "$RUNTIME.part" "$RUNTIME_URL"
  want="$(curl -fsSL "$RUNTIME_API" | python3 -c 'import json,sys
for a in json.load(sys.stdin).get("assets", []):
    if a["name"] == "runtime-x86_64": print((a.get("digest") or "").removeprefix("sha256:"))')"
  got="$(shasum -a 256 "$RUNTIME.part" | cut -d' ' -f1)"
  [ -n "$want" ] && [ "$want" = "$got" ] || { echo "empreinte du runtime incorrecte ($got ≠ $want)"; rm -f "$RUNTIME.part"; exit 1; }
  mv "$RUNTIME.part" "$RUNTIME"
fi

APPDIR="$WORK/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/mime/packages" \
  "$APPDIR/usr/share/metainfo" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp dist/linux/fablabcam-studio-ui dist/linux/fablabcam "$APPDIR/usr/bin/"
mkdir -p "$APPDIR/usr/share/licenses/fablabcam"
cp LICENSE NOTICE "$APPDIR/usr/share/licenses/fablabcam/"

python3 docs/make-icon.py "$WORK/icon-1024.png"
sips -z 256 256 "$WORK/icon-1024.png" --out "$APPDIR/usr/share/icons/hicolor/256x256/apps/$ID.png" >/dev/null
cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/$ID.png" "$APPDIR/$ID.png"
ln -s "$ID.png" "$APPDIR/.DirIcon"

cat > "$APPDIR/usr/share/applications/$ID.desktop" <<D
[Desktop Entry]
Type=Application
Name=FabLabCAM Studio
GenericName=FAO / CAM
Comment=Dessin 2D, trajectoires d'usinage et G-code
Exec=fablabcam-studio-ui %F
Icon=$ID
Terminal=false
Categories=Graphics;Engineering;
MimeType=application/x-fablabcam;image/vnd.dxf;image/svg+xml;
Keywords=CNC;G-code;CAM;FAO;usinage;
D
cp "$APPDIR/usr/share/applications/$ID.desktop" "$APPDIR/$ID.desktop"

cat > "$APPDIR/usr/share/mime/packages/$ID.xml" <<M
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-fablabcam">
    <comment>Projet FabLabCAM (.cb)</comment>
    <glob pattern="*.cb"/>
  </mime-type>
</mime-info>
M

cat > "$APPDIR/usr/share/metainfo/$ID.appdata.xml" <<X
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>$ID</id>
  <name>FabLabCAM Studio</name>
  <summary>Dessin 2D, trajectoires d'usinage et G-code</summary>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>Apache-2.0</project_license>
  <launchable type="desktop-id">$ID.desktop</launchable>
  <releases><release version="$VERSION"/></releases>
</component>
X

# AppRun : « cli » lance la ligne de commande, sinon l'interface (fichiers passés tels quels)
cat > "$APPDIR/AppRun" <<'R'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
if [ "$1" = "cli" ]; then shift; exec "$HERE/usr/bin/fablabcam" "$@"; fi
exec "$HERE/usr/bin/fablabcam-studio-ui" "$@"
R
chmod +x "$APPDIR/AppRun" "$APPDIR/usr/bin/"*

SFS="$WORK/app.squashfs"
rm -f "$SFS"
mksquashfs "$APPDIR" "$SFS" -comp zstd -root-owned -noappend -no-xattrs -quiet >/dev/null
mkdir -p "$OUT"
IMG="$OUT/FabLabCAM_Studio-$VERSION-x86_64.AppImage"
cat "$RUNTIME" "$SFS" > "$IMG"
chmod +x "$IMG"
ls -la "$IMG"
