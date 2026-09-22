#!/usr/bin/env python3
"""Regenerate menu thumbnails from the actual design renders listed in sources.json.

For missing starter renders first run:
  cargo run -p ringdesign-core --release --example template_shots -- target/template-library-review/starter-renders
Oriel's original render is produced by the oriel example into target/atelier/.
"""
from pathlib import Path
import hashlib
import json
from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "crates/ringdesign-workbench/assets/templates"
manifest = json.loads((DEST / "sources.json").read_text())
for slug, entry in manifest.items():
    source = ROOT / entry["source"]
    image = Image.open(source).convert("RGB")
    image = ImageOps.pad(image, (160, 160), method=Image.Resampling.LANCZOS, color=(10, 10, 14))
    image.save(DEST / f"{slug}.png", optimize=True)
    entry["source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
(DEST / "sources.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"Updated {len(manifest)} previews in {DEST.relative_to(ROOT)}")
