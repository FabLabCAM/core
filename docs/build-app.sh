#!/bin/bash
# Construit « FabLabCAM Studio.app » (double-clic dans le Finder, sans terminal).
# usage : docs/build-app.sh [dossier-de-sortie]      (défaut : dist/)
# Le .app est signé « ad hoc » (sans identité de développeur) : il fonctionne sur ce Mac ;
# copié sur un autre Mac, macOS demandera une confirmation au premier lancement.
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-dist}"
APP="$OUT/FabLabCAM Studio.app"
NAME="FabLabCAM-Studio"

cargo build --locked --release -p fablabcam-view --bin fablabcam-studio-ui
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/fablabcam-studio-ui "$APP/Contents/MacOS/$NAME"
cp LICENSE NOTICE "$APP/Contents/Resources/"

# icône : PNG 1024 → .icns (sips + iconutil sont fournis par macOS)
TMP="$(mktemp -d)"
python3 docs/make-icon.py "$TMP/icon.png"
mkdir "$TMP/icon.iconset"
for s in 16 32 64 128 256 512; do
  sips -z $s $s "$TMP/icon.png" --out "$TMP/icon.iconset/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$TMP/icon.png" --out "$TMP/icon.iconset/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$TMP/icon.iconset" -o "$APP/Contents/Resources/AppIcon.icns"
rm -rf "$TMP"

VERSION="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
cat > "$APP/Contents/Info.plist" <<P
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.fablabcam.studio</string>
<key>CFBundleName</key><string>FabLabCAM Studio</string>
<key>CFBundleDisplayName</key><string>FabLabCAM Studio</string>
<key>CFBundleExecutable</key><string>$NAME</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${VERSION:-0.1.0}</string>
<key>CFBundleVersion</key><string>${VERSION:-0.1.0}</string>
<key>LSMinimumSystemVersion</key><string>11.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>CFBundleDocumentTypes</key><array>
<dict><key>CFBundleTypeName</key><string>Projet FabLabCAM (.cb)</string><key>CFBundleTypeExtensions</key><array><string>cb</string></array><key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Owner</string></dict>
<dict><key>CFBundleTypeName</key><string>Dessin DXF</string><key>CFBundleTypeExtensions</key><array><string>dxf</string></array><key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Alternate</string></dict>
<dict><key>CFBundleTypeName</key><string>Dessin SVG</string><key>CFBundleTypeExtensions</key><array><string>svg</string></array><key>CFBundleTypeRole</key><string>Viewer</string><key>LSHandlerRank</key><string>Alternate</string></dict>
<dict><key>CFBundleTypeName</key><string>Post-processeur XML</string><key>CFBundleTypeExtensions</key><array><string>cbpp</string></array><key>CFBundleTypeRole</key><string>Viewer</string><key>LSHandlerRank</key><string>Alternate</string></dict>
</array>
<key>LSApplicationCategoryType</key><string>public.app-category.graphics-design</string>
</dict></plist>
P

codesign --force --sign - "$APP" >/dev/null 2>&1 || echo "avertissement : signature ad hoc impossible"
echo "$APP"
