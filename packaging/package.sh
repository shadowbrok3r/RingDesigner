#!/usr/bin/env bash
# Build RingDesigner for distribution and lay out what a jeweller receives.
#
#   packaging/package.sh linux           this machine's glibc — fast, for testing
#   packaging/package.sh linux-portable  glibc 2.36 in a container — what you send
#   packaging/package.sh windows         one .exe, static CRT
#   packaging/package.sh both            linux-portable + windows
#
# Every asset is inside the executable (crates/ringdesign-assets), so a
# package is the binary plus the desktop integration its platform wants.
# Nothing is read from a source tree at run time.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/ringdesign-gui/Cargo.toml | head -1)"
dist="$root/dist"
mkdir -p "$dist"

# The build script's PNGs for a target, newest first: a stale sibling
# directory from an earlier build must not win.
icons_for() {
  find "$root/target/${1:+$1/}release/build" -name 'icon-256.png' -path '*ringdesign-assets*' \
    -printf '%T@ %h\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-
}

# Lay out the tarball around whichever binary was built. `PORTABLE_BIN` is
# set by the container path; without it this builds for this machine.
build_linux() {
  local binary="${PORTABLE_BIN:-}"
  if [ -z "$binary" ]; then
    echo "==> linux x86_64 (this machine)"
    cargo build --release -p ringdesign-gui
    binary="target/release/ringdesigner"
  fi
  local stage="$dist/ringdesigner-$version-linux-x86_64"
  rm -rf "$stage"; mkdir -p "$stage/icons"
  cp "$binary" "$stage/ringdesigner"
  strip "$stage/ringdesigner" 2>/dev/null || true
  cp packaging/ringdesigner.desktop "$stage/"
  cp bundled/icon/ringdesigner.svg "$stage/icons/"
  local out; out="$(icons_for "")"
  [ -n "$out" ] && cp "$out"/icon-*.png "$stage/icons/" || true
  cp packaging/install.sh "$stage/"
  chmod +x "$stage/install.sh" "$stage/ringdesigner"
  tar -C "$dist" -czf "$stage.tar.gz" "$(basename "$stage")"
  echo "    $stage.tar.gz  ($(du -h "$stage.tar.gz" | cut -f1))"
  # A GUI cannot be statically linked here: glutin dlopens libGL/libEGL and
  # winit needs the X11/Wayland client libraries. What the binary does carry
  # is a glibc floor, set by whatever built it — and this workstation's is
  # newer than any distribution ships.
  local floor
  floor="$(objdump -T "$stage/ringdesigner" 2>/dev/null | sed -n 's/.*GLIBC_\([0-9.]*\).*/\1/p' | sort -V | tail -1)"
  if [ -n "$floor" ]; then
    echo "    needs glibc >= $floor"
    [ -z "${PORTABLE_BIN:-}" ] && echo "    this is a local build — run 'linux-portable' for one you can send out" || true
  fi
}

build_windows() {
  echo "==> windows x86_64 (msvc, static CRT)"
  # +crt-static comes from .cargo/config.toml, so the .exe needs no runtime
  # install. cargo-xwin supplies the MSVC SDK when building from Linux.
  if command -v cargo-xwin >/dev/null; then
    cargo xwin build --release -p ringdesign-gui --target x86_64-pc-windows-msvc
  else
    echo "    note: cargo-xwin not found — needs an MSVC SDK on this machine."
    echo "          cargo install cargo-xwin"
    cargo build --release -p ringdesign-gui --target x86_64-pc-windows-msvc
  fi
  local stage="$dist/ringdesigner-$version-windows-x86_64"
  rm -rf "$stage"; mkdir -p "$stage"
  cp target/x86_64-pc-windows-msvc/release/ringdesigner.exe "$stage/"
  (cd "$dist" && zip -qr "$(basename "$stage").zip" "$(basename "$stage")")
  echo "    $stage.zip  ($(du -h "$stage.zip" | cut -f1))"
}

# Export the texture gate's address and key from the first env file that has
# them, for a build that cannot read those files itself. Absent is fine: the
# build says so and ships without the feature.
gate_env() {
  [ -n "${COMFY_GATE_KEY:-}" ] && return 0
  local f
  for f in "${COMFY_ENV_FILE:-}" "$root/.env" "${XDG_CONFIG_HOME:-$HOME/.config}/ringdesigner/comfy.env"; do
    [ -n "$f" ] && [ -f "$f" ] || continue
    # shellcheck disable=SC2046
    export $(grep -E '^COMFY_GATE_(URL|KEY)=' "$f" | xargs -d '\n') 2>/dev/null || true
    [ -n "${COMFY_GATE_KEY:-}" ] && return 0
  done
  echo "    note: no COMFY_GATE_KEY found — this package will have no texture server"
}

# A binary built on this workstation needs its glibc, which is newer than any
# mainstream distribution ships — so it runs nowhere else. The container sets
# the floor at Debian 12's 2.36 and nothing else about the build changes.
build_linux_portable() {
  local engine
  engine="$(command -v podman || command -v docker)" || {
    echo "needs podman or docker for a portable build; 'linux' builds for this machine only" >&2
    exit 1
  }
  echo "==> linux x86_64 (portable, glibc 2.36)"
  "$engine" build -q -t ringdesigner-build:bookworm -f packaging/Dockerfile.linux packaging/
  # The container sees neither the workspace .env nor ~/.config, so the gate
  # credentials build.rs compiles in have to be handed across. `-e NAME` passes
  # the value through the environment rather than the command line, where a
  # process listing would show it.
  gate_env
  "$engine" run --rm \
    -e COMFY_GATE_URL -e COMFY_GATE_KEY \
    -v "$root":/src:ro \
    -v ringdesigner-build-target:/target \
    -v ringdesigner-build-cargo:/root/.cargo/registry \
    -e CARGO_TARGET_DIR=/target \
    -e RUSTFLAGS=-Awarnings \
    -e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc \
    ringdesigner-build:bookworm \
    cargo build --release --locked -p ringdesign-gui
  # The binary lives in the volume; copy it out through a throwaway container.
  mkdir -p "$root/target/portable"
  "$engine" run --rm -v ringdesigner-build-target:/target -v "$root/target/portable":/out \
    ringdesigner-build:bookworm cp /target/release/ringdesigner /out/ringdesigner
  PORTABLE_BIN="$root/target/portable/ringdesigner" build_linux
}

case "${1:-both}" in
  linux) build_linux ;;
  linux-portable) build_linux_portable ;;
  windows) build_windows ;;
  both) build_linux_portable; build_windows ;;
  *) echo "usage: $0 [linux|linux-portable|windows|both]" >&2; exit 2 ;;
esac
