#!/bin/bash
# Compile FabLabCAM Studio et la ligne de commande pour Windows et Linux (x86_64) depuis macOS.
# usage : docs/build-cross.sh [windows] [linux]      (sans argument : les deux)
#
# Prérequis (une fois) :
#   rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-gnu
#   brew install mingw-w64        # éditeur de liens Windows
#   brew install zig              # éditeur de liens Linux (zig cc), avec la glibc 2.28
# Les exécutables produits n'ont pas été essayés sur Windows ni sous Linux depuis ce Mac.
set -euo pipefail
cd "$(dirname "$0")/.."
targets=("$@")
[ ${#targets[@]} -eq 0 ] && targets=(windows linux)
bins=(-p fablabcam-view --bin fablabcam-studio-ui -p fablabcam-cli --bin fablabcam)

for t in "${targets[@]}"; do
  case "$t" in
    windows)
      export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
      cargo build --locked --release --target x86_64-pc-windows-gnu "${bins[@]}"
      mkdir -p dist/windows
      cp target/x86_64-pc-windows-gnu/release/fablabcam-studio-ui.exe target/x86_64-pc-windows-gnu/release/fablabcam.exe dist/windows/
      cp LICENSE NOTICE dist/windows/
      ls -la dist/windows
      ;;
    linux)
      # zig cc comme éditeur de liens ; la glibc 2.28 couvre les distributions courantes depuis 2018
      wrapper="$(mktemp -d)/zigcc"
      printf '#!/bin/bash\nexec zig cc -target x86_64-linux-gnu.2.28 "$@"\n' > "$wrapper"
      chmod +x "$wrapper"
      export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER="$wrapper"
      export CC_x86_64_unknown_linux_gnu="$wrapper"
      # sans Wayland natif : sa pile de dépendances récente exige un Rust plus récent que 1.75 (X11 / XWayland)
      cargo build --locked --release --target x86_64-unknown-linux-gnu --no-default-features "${bins[@]}"
      mkdir -p dist/linux
      cp target/x86_64-unknown-linux-gnu/release/fablabcam-studio-ui target/x86_64-unknown-linux-gnu/release/fablabcam dist/linux/
      cp LICENSE NOTICE dist/linux/
      ls -la dist/linux
      ;;
    *) echo "cible inconnue : $t (windows, linux)"; exit 1 ;;
  esac
done
