#!/usr/bin/env bash
# Put RingDesigner in this user's menu. Nothing needs root and nothing is
# installed system-wide — the program's own assets are in the binary.
#
#   ./install.sh            install for this user
#   ./install.sh --uninstall
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin="$HOME/.local/bin"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
# XDG paths must be absolute; use the standard location for an invalid override.
[[ "$data" = /* ]] || data="$HOME/.local/share"
apps="$data/applications"
icons="$data/icons/hicolor"
doc="$data/doc/ringdesigner"

# Exec has both desktop-entry string escaping and quoted-argument escaping.
# An absolute path also works when ~/.local/bin is absent from the desktop's PATH.
desktop_exec_argument() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//\`/\\\`}"
  value="${value//\$/\\\$}"
  value="${value//\\/\\\\}"
  value="${value//%/%%}"
  printf '"%s"' "$value"
}

desktop_exec_command() {
  case "$1" in
    *%*|*=*)
      # Launchers check program names before expanding %%; '=' is forbidden in
      # them. Pass these paths as arguments to a fixed, safely quoted command.
      printf 'sh -c %s ringdesigner %s' "$(desktop_exec_argument 'exec "$@"')" "$(desktop_exec_argument "$1")"
      ;;
    *) desktop_exec_argument "$1" ;;
  esac
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$bin/ringdesigner" "$apps/ringdesigner.desktop"
  rm -f "$icons"/*/apps/ringdesigner.png "$icons/scalable/apps/ringdesigner.svg"
  rm -rf "$doc"
  # The OpenCascade worker the app unpacked for itself goes with it, from the data folder the app uses.
  rm -rf "$data/ringdesigner/occt"
  echo "Removed. Your designs in $data/ringdesigner were left alone."
else
  for required in ringdesigner ringdesigner.desktop icons/ringdesigner.svg; do
    [ -f "$here/$required" ] || { echo "Missing $here/$required — run install.sh from the extracted Linux package." >&2; exit 1; }
  done
  mkdir -p "$bin" "$apps" "$icons/scalable/apps" "$doc"
  install -m755 "$here/ringdesigner" "$bin/ringdesigner"
  launcher="$(mktemp "$apps/.ringdesigner.XXXXXX.desktop")"
  trap 'rm -f "$launcher"' EXIT
  executable="$(desktop_exec_command "$bin/ringdesigner")"
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      Exec=ringdesigner*) printf 'Exec=%s%s\n' "$executable" "${line#Exec=ringdesigner}" ;;
      *) printf '%s\n' "$line" ;;
    esac
  done < "$here/ringdesigner.desktop" > "$launcher"
  if command -v desktop-file-validate >/dev/null; then desktop-file-validate "$launcher"; fi
  chmod 644 "$launcher"
  mv -f "$launcher" "$apps/ringdesigner.desktop"
  trap - EXIT
  for licence in LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md; do
    if [ -e "$here/$licence" ]; then install -m644 "$here/$licence" "$doc/$licence"; fi
  done
  install -m644 "$here/icons/ringdesigner.svg" "$icons/scalable/apps/ringdesigner.svg"
  for png in "$here"/icons/icon-*.png; do
    [ -e "$png" ] || continue
    edge="${png##*/icon-}"; edge="${edge%.png}"
    mkdir -p "$icons/${edge}x${edge}/apps"
    install -m644 "$png" "$icons/${edge}x${edge}/apps/ringdesigner.png"
  done
  echo "Installed to $bin/ringdesigner."
  echo "Launcher and icons installed in $data."
  case ":$PATH:" in *":$bin:"*) ;; *) echo "Note: $bin is not on your PATH." ;; esac
fi

command -v update-desktop-database >/dev/null && update-desktop-database -q "$apps" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$icons" 2>/dev/null || true
case "${XDG_CURRENT_DESKTOP:-}" in
  *KDE*)
    for cache in kbuildsycoca6 kbuildsycoca5; do
      if command -v "$cache" >/dev/null; then "$cache" --noincremental >/dev/null 2>&1 || true; break; fi
    done
    ;;
esac
