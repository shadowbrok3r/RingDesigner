#!/usr/bin/env bash
# Put RingDesigner in this user's menu. Nothing needs root and nothing is
# copied anywhere but ~/.local — the program's own assets are in the binary.
#
#   ./install.sh            install for this user
#   ./install.sh --uninstall
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
bin="$HOME/.local/bin"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor"
doc="$HOME/.local/share/doc/ringdesigner"

if [ "${1:-}" = "--uninstall" ]; then
  rm -f "$bin/ringdesigner" "$apps/ringdesigner.desktop"
  rm -f "$icons"/*/apps/ringdesigner.png "$icons/scalable/apps/ringdesigner.svg"
  rm -rf "$doc"
  # The OpenCascade worker the app unpacked for itself goes with it.
  rm -rf "$HOME/.local/share/ringdesigner/occt"
  echo "Removed. Your designs in ~/.local/share/ringdesigner were left alone."
else
  mkdir -p "$bin" "$apps" "$icons/scalable/apps" "$doc"
  install -m755 "$here/ringdesigner" "$bin/ringdesigner"
  install -m644 "$here/ringdesigner.desktop" "$apps/ringdesigner.desktop"
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
  case ":$PATH:" in *":$bin:"*) ;; *) echo "Note: $bin is not on your PATH." ;; esac
fi

command -v update-desktop-database >/dev/null && update-desktop-database -q "$apps" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -qtf "$icons" 2>/dev/null || true
