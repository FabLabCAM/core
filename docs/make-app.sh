#!/bin/bash
# Enveloppe un binaire du workspace dans un bundle .app (id org.fablabcam.view) pour pouvoir le capturer
# avec computer-use. usage : docs/make-app.sh <binaire> <dossier-sortie> [arguments du binaire...]
BIN="$1"; OUT="$2"; shift 2
A="$OUT/FabLabCAMView.app"
pkill -f fablabcam-view-bin 2>/dev/null; rm -rf "$A"
mkdir -p "$A/Contents/MacOS"
cp "$BIN" "$A/Contents/MacOS/fablabcam-view-bin"
{ echo '#!/bin/bash'; echo 'export FABLABCAM_PREFS=/tmp/fablabcam-test-prefs.txt'; echo 'cd /tmp  # copie de test : pas d'\''accès au dossier Documents'; echo 'D="$(cd "$(dirname "$0")" && pwd)"'; printf 'exec "$D/fablabcam-view-bin"'; for x in "$@"; do printf " %q" "$x"; done; echo; } > "$A/Contents/MacOS/FabLabCAMView"
chmod +x "$A/Contents/MacOS/FabLabCAMView"
cat > "$A/Contents/Info.plist" <<'P'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>org.fablabcam.view</string>
<key>CFBundleName</key><string>FabLabCAMView</string>
<key>CFBundleExecutable</key><string>FabLabCAMView</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>NSHighResolutionCapable</key><true/>
<key>CFBundleDocumentTypes</key><array>
<dict><key>CFBundleTypeName</key><string>Projet FabLabCAM (.cb)</string><key>CFBundleTypeExtensions</key><array><string>cb</string></array><key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Owner</string></dict>
<dict><key>CFBundleTypeName</key><string>Dessin DXF</string><key>CFBundleTypeExtensions</key><array><string>dxf</string></array><key>CFBundleTypeRole</key><string>Editor</string><key>LSHandlerRank</key><string>Alternate</string></dict>
<dict><key>CFBundleTypeName</key><string>Dessin SVG</string><key>CFBundleTypeExtensions</key><array><string>svg</string></array><key>CFBundleTypeRole</key><string>Viewer</string><key>LSHandlerRank</key><string>Alternate</string></dict>
<dict><key>CFBundleTypeName</key><string>Post-processeur XML</string><key>CFBundleTypeExtensions</key><array><string>cbpp</string></array><key>CFBundleTypeRole</key><string>Viewer</string><key>LSHandlerRank</key><string>Alternate</string></dict>
</array>
</dict></plist>
P
open -g "$A"
