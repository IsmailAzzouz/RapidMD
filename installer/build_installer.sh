#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE=fast
VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)"
if [[ -z "$VERSION" ]]; then echo "Could not read package version from Cargo.toml" >&2; exit 1; fi
case "$(uname -m)" in
  x86_64|amd64) ARCH=x86_64 ;;
  aarch64|arm64) ARCH=aarch64 ;;
  *) echo "Unsupported Linux architecture: $(uname -m)" >&2; exit 1 ;;
esac

cargo build --locked --profile "$PROFILE" --bin rapidmd
BINARY="$ROOT/target/$PROFILE/rapidmd"
[[ -x "$BINARY" ]] || { echo "Build did not produce $BINARY" >&2; exit 1; }
OUT="$ROOT/installer/dist"
STAGE="$OUT/rapidmd-linux-${ARCH}-staging"
PACKAGE="$OUT/RapidMD-v${VERSION}-linux-${ARCH}.tar.gz"
rm -rf "$STAGE"
mkdir -p "$STAGE"
install -m 0755 "$BINARY" "$STAGE/rapidmd"
install -m 0755 "$ROOT/installer/install-linux.sh" "$STAGE/install-linux.sh"
install -m 0644 "$ROOT/assets/Icon/RMD.png" "$STAGE/RMD.png"
cat > "$STAGE/README.txt" <<DOC
RapidMD v${VERSION} for Linux (${ARCH})

Extract this archive and run:
  ./install-linux.sh

This installs RapidMD for the current user, adds an application launcher and Desktop shortcut,
and registers Markdown MIME types for the file manager's Open With menu. No sudo is needed.
To uninstall, run:
  ./install-linux.sh --uninstall

Required GUI libraries are those used by eframe/winit and GTK file dialogs. On Ubuntu, install
libwayland-client0 libxkbcommon0 libxkbcommon-x11-0 libxcb1 libx11-6 libgtk-3-0 if missing.
DOC
rm -f "$PACKAGE"
tar -C "$OUT" -czf "$PACKAGE" "$(basename "$STAGE")/rapidmd" "$(basename "$STAGE")/install-linux.sh" "$(basename "$STAGE")/RMD.png" "$(basename "$STAGE")/README.txt"
rm -rf "$STAGE"
echo "Created $PACKAGE"
if [[ "${1:-}" == "--install" ]]; then
  TMP="$(mktemp -d)"
  trap 'rm -rf "$TMP"' EXIT
  tar -xzf "$PACKAGE" -C "$TMP"
  "$TMP/rapidmd-linux-${ARCH}-staging/install-linux.sh"
fi
