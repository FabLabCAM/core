#!/bin/bash
# Capture d'une page du serveur local (port 8765) en PNG 1360x860 avec Chrome sans interface.
# usage : docs/shot.sh "<chemin?query>" sortie.png
CH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
"$CH" --headless=new --disable-gpu --hide-scrollbars --window-size=1360,860 --virtual-time-budget=4000 --screenshot="$2" "http://localhost:8765/$1" >/dev/null 2>&1
