#!/usr/bin/env python3
"""Regenerate menu thumbnails from the actual design renders listed in sources.json.

For missing starter renders first run (finished rings in studio gold, stones set, CAD ones included):
  cargo run -p ringdesign-core --release --example template_shots -- target/template-library-review/starter-renders
Oriel's original render is produced by the oriel example into target/atelier/.
A source missing from disk is named and skipped; the rest still regenerate.
Name slugs to regenerate only those: python3 tools/template_thumbnails.py arachne-bestiarium
"""
from pathlib import Path
import hashlib
import json
import sys
from PIL import Image, ImageOps

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / "crates/ringdesign-workbench/assets/templates"
manifest = json.loads((DEST / "sources.json").read_text())
wanted = set(sys.argv[1:])
unknown = wanted - manifest.keys()
if unknown:
    sys.exit(f"Not in sources.json: {', '.join(sorted(unknown))}")
missing = []
for slug, entry in manifest.items():
    if wanted and slug not in wanted:
        continue
    source = ROOT / entry["source"]
    if not source.is_file():
        missing.append(slug)
        continue
    image = Image.open(source).convert("RGB")
    image = ImageOps.pad(image, (160, 160), method=Image.Resampling.LANCZOS, color=(10, 10, 14))
    image.save(DEST / f"{slug}.png", optimize=True)
    entry["source_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
(DEST / "sources.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"Updated {len(wanted or manifest) - len(missing)} previews in {DEST.relative_to(ROOT)}")
for slug in missing:
    print(f"  {slug}: no render at {manifest[slug]['source']}")
