#!/usr/bin/env bash
# Build RingDesigner for distribution and lay out what a jeweller receives.
#
#   packaging/package.sh linux           this machine's glibc — fast, for testing
#   packaging/package.sh linux-portable  glibc 2.36 in a container — what you send
#   packaging/package.sh windows         one .exe, static CRT
#   packaging/package.sh both            linux-portable + windows
#
# Add --occt to any of them to carry OpenCascade: the worker is built for the
# target first (Linux natively or in the container, Windows with the MinGW
# target, since cadrum's MSVC OpenCascade wants the dynamic CRT the app does
# not use), stripped, and handed to the app's build.rs through
# RINGDESIGNER_OCCT_WORKER, which deflates it into the executable. The app
# unpacks it into its data folder the first time a fillet, a shell, a junction
# or a STEP import asks for it. Without --occt nothing changes.
#
# macOS is built on a Mac and not here: there the same two steps are
#   cargo build --release -p ringdesign-occt --bin occt-worker --features kernel-occt --target aarch64-apple-darwin
#   strip -x target/aarch64-apple-darwin/release/occt-worker   # then: codesign -s - -f it
#   RINGDESIGNER_OCCT_WORKER=target/aarch64-apple-darwin/release/occt-worker \
#     cargo build --release -p ringdesign-gui --target aarch64-apple-darwin
# (and x86_64-apple-darwin for Intel). A stripped Mach-O loses its ad-hoc
# signature, and Apple silicon refuses to run an unsigned one.
#
# Every asset is inside the executable (crates/ringdesign-assets), so a
# package is the binary plus the desktop integration its platform wants and
# the licences. Nothing is read from a source tree at run time.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' crates/ringdesign-gui/Cargo.toml | head -1)"
dist="$root/dist"
mkdir -p "$dist"

what="both"
occt=0
for arg in "$@"; do
  case "$arg" in
    --occt) occt=1 ;;
    linux|linux-portable|windows|both) what="$arg" ;;
    *) echo "usage: $0 [linux|linux-portable|windows|both] [--occt]" >&2; exit 2 ;;
  esac
done

# The licences travel beside the program as well as inside it.
licences() {
  cp LICENSE-MIT LICENSE-APACHE THIRD-PARTY-NOTICES.md "$1/"
}

# The build script's PNGs for a target, newest first: a stale sibling
# directory from an earlier build must not win.
icons_for() {
  find "$root/target/${1:+$1/}release/build" -name 'icon-256.png' -path '*ringdesign-assets*' \
    -printf '%T@ %h\n' 2>/dev/null | sort -rn | head -1 | cut -d' ' -f2-
}

mb() { echo "$(( $(stat -c %s "$1") / 1000000 )) MB"; }

# The Linux worker, built here and stripped where the app's build.rs reads it.
worker_linux() {
  echo "==> OpenCascade worker, linux x86_64 (this machine)"
  cargo build --release -p ringdesign-occt --bin occt-worker --features kernel-occt
  mkdir -p target/occt-embed/linux
  strip -o target/occt-embed/linux/occt-worker target/release/occt-worker
  echo "    target/occt-embed/linux/occt-worker  ($(mb target/occt-embed/linux/occt-worker) stripped)"
  export RINGDESIGNER_OCCT_WORKER="$root/target/occt-embed/linux/occt-worker"
}

# The Windows worker: cadrum's MinGW OpenCascade links the GCC runtime
# statically, so the .exe needs only system DLLs — which is checked here, since
# a worker that asked for libstdc++-6.dll would fail on every jeweller's PC.
worker_windows() {
  echo "==> OpenCascade worker, windows x86_64 (MinGW, static)"
  CARGO_TARGET_DIR="$root/target/occt-mingw" \
    cargo build --release -p ringdesign-occt --bin occt-worker --features kernel-occt --target x86_64-pc-windows-gnu
  mkdir -p target/occt-embed/windows
  x86_64-w64-mingw32-strip -o target/occt-embed/windows/occt-worker.exe \
    target/occt-mingw/x86_64-pc-windows-gnu/release/occt-worker.exe
  local dlls foreign
  dlls="$(x86_64-w64-mingw32-objdump -p target/occt-embed/windows/occt-worker.exe | sed -n 's/^\s*DLL Name: //p' | tr 'A-Z' 'a-z' | sort -u)"
  foreign="$(echo "$dlls" | grep -v -E '^(api-ms-win-.*|kernel32|ntdll|user32|advapi32|userenv|ws2_32|bcrypt|bcryptprimitives|shell32|ole32|oleaut32|msvcrt|secur32|crypt32|psapi|dbghelp)\.dll$' || true)"
  echo "    imports: $(echo "$dlls" | tr '\n' ' ')"
  if [ -n "$foreign" ]; then
    echo "the Windows worker needs DLLs a PC does not have: $foreign" >&2
    exit 1
  fi
  echo "    target/occt-embed/windows/occt-worker.exe  ($(mb target/occt-embed/windows/occt-worker.exe) stripped)"
  export RINGDESIGNER_OCCT_WORKER="$root/target/occt-embed/windows/occt-worker.exe"
}

# Lay out the tarball around whichever binary was built. `PORTABLE_BIN` is
# set by the container path; without it this builds for this machine.
build_linux() {
  local binary="${PORTABLE_BIN:-}"
  if [ -z "$binary" ]; then
    [ "$occt" = 1 ] && worker_linux
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
  licences "$stage"
  chmod +x "$stage/install.sh" "$stage/ringdesigner"
  tar -C "$dist" -czf "$stage.tar.gz" "$(basename "$stage")"
  echo "    ringdesigner $(mb "$stage/ringdesigner") stripped; $stage.tar.gz  ($(du -h "$stage.tar.gz" | cut -f1))"
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
  if [ -n "${RINGDESIGNER_OCCT_WORKER:-}" ]; then
    floor="$(objdump -T "$RINGDESIGNER_OCCT_WORKER" 2>/dev/null | sed -n 's/.*GLIBC_\([0-9.]*\).*/\1/p' | sort -V | tail -1)"
    local cxx; cxx="$(objdump -T "$RINGDESIGNER_OCCT_WORKER" 2>/dev/null | sed -n 's/.*GLIBCXX_\([0-9.]*\).*/\1/p' | sort -V | tail -1)"
    echo "    carries OpenCascade; its worker needs glibc >= $floor and libstdc++ (GLIBCXX) >= $cxx"
  fi
}

build_windows() {
  [ "$occt" = 1 ] && worker_windows
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
  licences "$stage"
  (cd "$dist" && rm -f "$(basename "$stage").zip" && zip -qr "$(basename "$stage").zip" "$(basename "$stage")")
  echo "    ringdesigner.exe $(mb "$stage/ringdesigner.exe"); $stage.zip  ($(du -h "$stage.zip" | cut -f1))"
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
# the floor at Debian 12's 2.36 and nothing else about the build changes. The
# worker is built in the same container, so its floor is the app's.
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
  local build="cargo build --release --locked -p ringdesign-gui"
  if [ "$occt" = 1 ]; then
    build="cargo build --release --locked -p ringdesign-occt --bin occt-worker --features kernel-occt \
      && mkdir -p /target/occt-embed && strip -o /target/occt-embed/occt-worker /target/release/occt-worker \
      && RINGDESIGNER_OCCT_WORKER=/target/occt-embed/occt-worker $build"
  fi
  "$engine" run --rm \
    -e COMFY_GATE_URL -e COMFY_GATE_KEY \
    -v "$root":/src:ro \
    -v ringdesigner-build-target:/target \
    -v ringdesigner-build-cargo:/root/.cargo/registry \
    -e CARGO_TARGET_DIR=/target \
    -e RUSTFLAGS=-Awarnings \
    -e CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=cc \
    ringdesigner-build:bookworm \
    sh -c "$build"
  # The binaries live in the volume; copy them out through a throwaway container.
  mkdir -p "$root/target/portable"
  "$engine" run --rm -v ringdesigner-build-target:/target -v "$root/target/portable":/out \
    ringdesigner-build:bookworm sh -c "cp /target/release/ringdesigner /out/ringdesigner && { [ ! -f /target/occt-embed/occt-worker ] || cp /target/occt-embed/occt-worker /out/occt-worker; }"
  if [ "$occt" = 1 ]; then
    RINGDESIGNER_OCCT_WORKER="$root/target/portable/occt-worker" PORTABLE_BIN="$root/target/portable/ringdesigner" build_linux
  else
    PORTABLE_BIN="$root/target/portable/ringdesigner" build_linux
  fi
}

case "$what" in
  linux) build_linux ;;
  linux-portable) build_linux_portable ;;
  windows) build_windows ;;
  both) build_linux_portable; build_windows ;;
esac
