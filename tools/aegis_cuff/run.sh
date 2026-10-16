#!/usr/bin/env bash
# Rebuilds the Aegis-detailed trident cuff from the repository root; the work folder holds the layout and the three gate.py height maps.
set -euo pipefail
T="$(cd "$(dirname "$0")" && pwd)"
STL="${1:?path to 'Meshy_AI_Aegis of Olympus_1791926034_generate.stl'}"
W="target/cuff-aegis/work"
PY="${PYTHON:-python3}"
mkdir -p "$W" && cd "$W"
G="systemd-run --user --scope -p MemoryMax=24G --quiet --"
$G $PY -E -P "$T/polar.py" "$T" "$STL" aegis_welded.npz
$G $PY -E -P "$T/raster.py" "$T" "$STL" aegis_welded.npz
$PY -E -P "$T/groundfit.py"
$PY -E -P "$T/masks2.py"
for spec in "snake 0" "female 0.12" "center 0.30"; do
  set -- $spec
  $G $PY -E -P "$T/voxel2.py" "$T" "$STL" aegis_welded.npz "$1" 0.04 "$2"
done
$PY -E -P "$T/export_bin.py"
$G $PY -E -P "$T/compose.py"
cd - >/dev/null
systemd-run --user --scope -p MemoryMax=48G --quiet -- cargo run --release -p ringdesign-core --example cuff_aegis -- "$W" target/cuff-aegis --step 0.052 --keep 0.22
