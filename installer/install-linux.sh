#!/usr/bin/env bash
set -euo pipefail

APP_ID=rapidmd
APP_NAME=RapidMD
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_HOME="${XDG_BIN_HOME:-$HOME/.local/bin}"
APP_DIR="$DATA_HOME/applications"
SUPPORT_DIR="$DATA_HOME/rapidmd"
ICON_DIR="$DATA_HOME/icons/hicolor/256x256/apps"
DESKTOP_DIR="$(xdg-user-dir DESKTOP 2>/dev/null || true)"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -f "$BIN_HOME/$APP_ID" "$APP_DIR/$APP_ID.desktop" "$ICON_DIR/$APP_ID.png" "$SUPPORT_DIR/install-linux.sh"
  rmdir "$SUPPORT_DIR" 2>/dev/null || true
  if [[ -n "$DESKTOP_DIR" ]]; then rm -f "$DESKTOP_DIR/$APP_NAME.desktop"; fi
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APP_DIR" || true
  echo "$APP_NAME removed from this user account."
  exit 0
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "$SCRIPT_DIR/rapidmd" ]]; then
  SOURCE_BIN="$SCRIPT_DIR/rapidmd"
elif [[ -x "$SCRIPT_DIR/../target/fast/rapidmd" ]]; then
  SOURCE_BIN="$SCRIPT_DIR/../target/fast/rapidmd"
else
  echo "Could not find rapidmd beside this script or in target/fast/. Build it with installer/build_installer.sh first." >&2
  exit 1
fi
if [[ -f "$SCRIPT_DIR/RMD.png" ]]; then
  SOURCE_ICON="$SCRIPT_DIR/RMD.png"
elif [[ -f "$SCRIPT_DIR/../assets/Icon/RMD.png" ]]; then
  SOURCE_ICON="$SCRIPT_DIR/../assets/Icon/RMD.png"
else
  echo "Could not find RMD.png beside this script or in assets/Icon/." >&2
  exit 1
fi

install -d "$BIN_HOME" "$APP_DIR" "$ICON_DIR" "$SUPPORT_DIR"
install -m 0755 "$SOURCE_BIN" "$BIN_HOME/$APP_ID"
install -m 0644 "$SOURCE_ICON" "$ICON_DIR/$APP_ID.png"
install -m 0755 "$SCRIPT_DIR/install-linux.sh" "$SUPPORT_DIR/install-linux.sh"

# Escape backslashes and quotes for a Desktop Entry Exec string.
ESCAPED_BIN=${BIN_HOME%/}/$APP_ID
ESCAPED_BIN=${ESCAPED_BIN//\\/\\\\}
ESCAPED_BIN=${ESCAPED_BIN//\"/\\\"}
cat > "$APP_DIR/$APP_ID.desktop" <<DESKTOP
[Desktop Entry]
Version=1.0
Type=Application
Name=$APP_NAME
Comment=View and edit Markdown documents
Exec="$ESCAPED_BIN" %F
Icon=$ICON_DIR/$APP_ID.png
Terminal=false
StartupNotify=true
StartupWMClass=rapidmd
Categories=Office;
MimeType=text/markdown;text/x-markdown;
DESKTOP
chmod 0644 "$APP_DIR/$APP_ID.desktop"

if [[ -z "$DESKTOP_DIR" ]]; then
  DESKTOP_DIR="$HOME/Desktop"
fi
if [[ -d "$DESKTOP_DIR" ]]; then
  install -m 0755 "$APP_DIR/$APP_ID.desktop" "$DESKTOP_DIR/$APP_NAME.desktop"
  if command -v gio >/dev/null 2>&1; then
    gio set "$DESKTOP_DIR/$APP_NAME.desktop" metadata::trusted true 2>/dev/null || true
  fi
fi

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APP_DIR" || true
command -v desktop-file-validate >/dev/null 2>&1 && desktop-file-validate "$APP_DIR/$APP_ID.desktop"
cat <<MSG
$APP_NAME installed for $USER.
Application: $BIN_HOME/$APP_ID
Launcher:    $APP_DIR/$APP_ID.desktop
Desktop:     ${DESKTOP_DIR:-not available}
Markdown files can be opened with RapidMD from the file manager's Open With menu.
To make RapidMD the default for Markdown: xdg-mime default rapidmd.desktop text/markdown
To remove it: $SUPPORT_DIR/install-linux.sh --uninstall
MSG
