#!/usr/bin/env python3
"""Create the local jewelry inspection gallery from real model renders."""
import base64
import html
import json
from pathlib import Path
import shutil
import sys
from urllib.parse import quote
import zipfile

root = Path(sys.argv[1]).resolve()
workspace = Path(__file__).resolve().parents[1]
rows = json.loads((root / "collection.json").read_text())
assert {r["slug"] for r in rows} == {"solstice", "nocturne"}
shutil.copyfile(workspace / "assets/fonts/EBGaramond.ttf", root / "EBGaramond.ttf")
shutil.copyfile(workspace / "assets/fonts/OFL.txt", root / "FONT-LICENSE.txt")

copy = {
    "solstice": ("Solstice", "A solar seal in sterling silver", "Sand casting", "A raised solar disc, twelve broad rays with withdrawal stock, lotus cheeks, fluted shoulders and palm, woven side vines, beading, and warped sunseed tessellation. The ornament is part of the cast pattern; fine satin texture is a separate bench finish."),
    "nocturne": ("Nocturne", "A night garden in yellow gold", "Investment casting", "An eight-petal flower surrounds a green centre stone inside a double octagonal frame. Drawn stars, pearl edging, acanthus shoulders, two smaller stones, protected guilloche engraving, recessed galleries, and a palm inscription complete the design."),
}
views = [("hero", "Three-quarter"), ("seal", "Seal detail"), ("cheek", "Cheeks"), ("palm", "Palm"), ("reverse", "Reverse"), ("structure", "Base form"), ("pattern", "Casting pattern"), ("turntable", "Rotate")]
articles = []
sheet = ['<svg xmlns="http://www.w3.org/2000/svg" width="1800" height="1230" viewBox="0 0 1800 1230"><rect width="1800" height="1230" fill="#111"/><text x="64" y="83" fill="#eeeae0" font-family="serif" font-size="49">Solstice &amp; Nocturne</text><text x="65" y="123" fill="#aeaba3" font-family="sans-serif" font-size="21">Two authored signets · actual RingDesigner geometry · 18.200 mm nominal bore</text>']
readme = ["# Solstice & Nocturne", "", "Two original signets authored with RingDesigner’s existing profile, layer, artwork, stone-setting and manufacturing tools. Open `index.html` to inspect each view, compare the undecorated structure and casting pattern, play the turntable, and download the source.", "", "All imagery renders actual geometry. Green is the intended stone colour; polished metal and satin/oxidized recesses are finishing choices. Neither ring has been physically cast.", "", "| Design | Route | Approximate cast metal | Editable layers | Radial wall screen |", "|---|---|---:|---:|---:|"]

ring_notes = []

def compress_3mf(file):
    # The app writes portable stored ZIP members. Compress the same XML
    # payloads for delivery; preserve and check every member's CRC.
    with zipfile.ZipFile(file) as source:
        if all(i.compress_type == zipfile.ZIP_DEFLATED for i in source.infolist()):
            return
        original = {i.filename: i.CRC for i in source.infolist()}
        temporary = file.with_suffix(".3mf.packing")
        with zipfile.ZipFile(temporary, "x", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as target:
            for entry in source.infolist():
                target.writestr(entry.filename, source.read(entry.filename))
    with zipfile.ZipFile(temporary) as checked:
        assert {i.filename: i.CRC for i in checked.infolist()} == original
        assert checked.testzip() is None
    temporary.replace(file)

def layers(stack):
    for entry in stack["layers"]:
        yield entry
        group = entry["layer"].get("Group")
        if group:
            yield from layers(group["stack"])

for index, row in enumerate(rows):
    slug = row["slug"]
    folder = root / slug
    design = json.loads((folder / "design.ring.json").read_text())
    report = json.loads((folder / "report.json").read_text())
    wall = json.loads((folder / "wall-screen.json").read_text())
    entries = list(layers(design["layers"]))
    title, subtitle, process, description = copy[slug]
    for file in [folder / "nominal.3mf", folder / "pattern-package/pattern.3mf"]:
        compress_3mf(file)
    with zipfile.ZipFile(folder / "pattern-package.zip", "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
        for path in sorted((folder / "pattern-package").rglob("*")):
            if path.is_file():
                archive.write(path, path.relative_to(folder / "pattern-package"))
    names = sorted({next(iter(e["layer"])) for e in entries})
    inventory = "".join(f'<li><span>{html.escape(e["name"])}</span><small>{html.escape(next(iter(e["layer"]))) }{" · bench" if e.get("bench_only") else ""}</small></li>' for e in entries)
    nav = "".join(f'<button type="button" data-view="{tag}" aria-pressed="{str(tag == "hero").lower()}">{label}</button>' for tag, label in views)
    links = [("Editable ring project", "design.ring.json"), ("Casting package ZIP", "pattern-package.zip"), ("Nominal STL", "nominal.stl"), ("Nominal 3MF", "nominal.3mf"), ("3D preview GLB", "preview-metal.glb"), ("Manufacturing sheet", "pattern-package/molding-sheet.html")]
    if (folder / "setting-map.svg").exists():
        links += [("Stone-setting map", "setting-map.svg")]
    downloads = "".join(f'<a href="{slug}/{path}">{label}</a>' for label, path in links)
    samples = ["Palmette.svg", "Solar-rays-with-withdrawal-stock.png", "Sunseed-tile.svg", "Shoulder-mask.svg"] if slug == "solstice" else ["Night-bloom.svg", "Seal-frame.svg", "Palmette.svg", "Drawn-evening-star.png", "Engraving-resist.png", "Nocturne-signature.png"]
    art = "".join(f'<a class="art" href="{slug}/artwork/{quote(name)}"><img loading="lazy" src="{slug}/artwork/{quote(name)}" alt="{html.escape(Path(name).stem.replace("-", " "))}"><span>{html.escape(Path(name).stem.replace("-", " "))}</span></a>' for name in samples if (folder / "artwork" / name).exists())
    findings = report["detail_findings"]
    if slug == "solstice":
        finding = "Zero sampled withdrawal obstructions at 0.100 and 0.075 mm; no detail-size warnings. Physical pull trial required."
        notes = "Withdraw the two mold halves along ±Z. Cast the complete solar, lotus, fluted and beaded ornament. Finish the fine satin texture at the bench. The solar alpha includes supports toward the parting line so its rays do not trap sand."
    else:
        finding = f"Investment pattern; three separate stones. {len(findings)} fine-detail advisories remain for the caster."
        notes = "Cast the metal body in one piece from a sacrificial pattern. The gallery tool creates deep recesses with retained floors, not through-holes. Set one 2.3 mm and two 1.5 mm round stones after cutting actual bearings and pavilion clearance. Engrave NOCTURNE on the palm at the bench."
    wall_note = f'The additional coarse surface-normal screen sampled {wall["rays"]} rays: minimum {wall["sampled_min_mm"]:.3f} mm, {wall["below_limit"]} below the {wall["limit_mm"]:.2f} mm general limit. Inspect these edge/detail locations in the source; this is a separate measurement from the radial wall screen.'
    warning_list = '<ul class="advisories">'+"".join(f'<li>{html.escape(x)}</li>' for x in findings)+'</ul>' if findings else ''
    articles.append(f'''<article id="{slug}" data-ring="{slug}">
<div class="visual"><div class="photo"><img class="ring" src="{slug}/hero.png" alt="{title}, actual three-quarter model render" width="1400" height="1400"></div><nav aria-label="{title} views">{nav}</nav><p class="caption">Actual geometry with intended metal and stone colours. Select a view to inspect.</p></div>
<div class="description"><p class="route">{process}</p><h2>{title}</h2><p class="subtitle">{subtitle}</p><p>{description}</p>
<dl><div><dt>Cast metal estimate</dt><dd>{row["cast_grams"]:.2f} g</dd></div><div><dt>Editable layers</dt><dd>{len(entries)}</dd></div><div><dt>Radial wall screen</dt><dd>{row["radial_wall_mm"]:.3f} mm</dd></div></dl><p class="finding">{finding}</p><div class="downloads">{downloads}</div>
<details><summary>Tools and layer stack</summary><p>{', '.join(names)}. Artwork also uses SVG import, procedural generators, masks, mirroring, height remapping and explicit casting stages.</p><ol class="layers">{inventory}</ol></details>
<details><summary>Source artwork and masks</summary><div class="artwork">{art}</div><p>All referenced artwork is included in the ring project. SVG, text, brush strokes and procedural recipes remain editable; derived support and resist masks are embedded.</p></details>
<details><summary>Manufacturing notes and checks</summary><p>{notes}</p><p>{wall_note}</p><p>Weights are before fitting stone pockets and final finishing. Shrink allowances are starting values that require shop calibration. Low-draft surfaces and small unsampled features still need workshop review.</p>{warning_list}<div class="downloads"><a href="{slug}/report.json">Full report</a><a href="{slug}/wall-screen.json">Local thickness screen</a><a href="{slug}/verification.json">Source identity checks</a><a href="{slug}/sand-orientations.json">Six sand-pull orientations</a></div></details></div></article>''')
    image_data = base64.b64encode((folder / "hero.png").read_bytes()).decode()
    x = 45 + index * 880
    sheet.append(f'<image x="{x}" y="158" width="830" height="830" href="data:image/png;base64,{image_data}"/><text x="{x+25}" y="1025" font-family="serif" font-size="48" fill="#eeeae0">{title}</text><text x="{x+25}" y="1063" font-family="sans-serif" font-size="21" fill="#d3ad69">{html.escape(process)} · {row["alloy"]} · {row["cast_grams"]:.2f} g</text><text x="{x+25}" y="1100" font-family="sans-serif" font-size="19" fill="#aeaba3">{len(entries)} editable layers · original alphas, relief and textures</text>')
    readme.append(f'| {title} | {process} | {row["cast_grams"]:.2f} g | {len(entries)} | {row["radial_wall_mm"]:.3f} mm |')
    ring_notes += ["", f"## {title}", "", notes, "", finding, "", wall_note, ""]

page = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Solstice & Nocturne — RingDesigner</title><style>
@font-face{font-family:Atelier;src:url(EBGaramond.ttf)}*{box-sizing:border-box}body{margin:0;background:#111;color:#eeeae0;font:16px/1.6 system-ui,sans-serif}header,main,footer{max-width:1450px;margin:auto;padding:30px 48px}header{padding-top:46px;display:flex;gap:32px;align-items:baseline;justify-content:space-between}h1,h2,.subtitle{font-family:Atelier,Georgia,serif;font-weight:400;margin:0}h1{font-size:46px;line-height:1.1}header p{color:#aeaba3;max-width:38ch;margin:0}header a{color:#d3ad69}article{display:grid;grid-template-columns:minmax(0,1.2fr) minmax(340px,.8fr);gap:32px;padding:25px 0 70px}article+article{border-top:1px solid #36342e;padding-top:60px}.visual{min-width:0}.photo{overflow:hidden;aspect-ratio:1}.ring{width:100%;height:100%;object-fit:contain;display:block;transition:transform .18s ease}.photo.detail .ring{transform:scale(1.65)}nav{display:flex;flex-wrap:wrap;gap:4px}button{font:inherit;font-size:14px;color:#aeaba3;background:transparent;min-height:44px;padding:8px 12px;border:1px solid transparent;border-radius:4px;cursor:pointer}button[aria-pressed=true]{color:#eeeae0;border-color:#71654e;background:#211f19}button:hover{color:#eeeae0}.caption{font-size:13px;color:#aeaba3;margin:12px 8px}.route{margin:14px 0 0;color:#d3ad69;font-size:14px}h2{font-size:68px;line-height:1.05;margin-top:8px}.subtitle{font-size:27px;color:#aeaba3;line-height:1.3;margin:8px 0 25px}.description>p{max-width:62ch}.description p:not(.subtitle){font-size:15px}dl{display:flex;gap:25px;flex-wrap:wrap;margin:24px 0}dt{font-size:12px;color:#aeaba3}dd{margin:2px 0 0;font-variant-numeric:tabular-nums;font-size:20px}.finding{color:#99b3a2}.downloads{display:flex;flex-wrap:wrap;gap:8px 20px}.downloads a{font-size:14px;color:#dbc18e;text-underline-offset:4px}details{margin-top:20px}summary{cursor:pointer;min-height:44px;padding-top:10px;color:#eeeae0}details p{color:#aeaba3}.layers{padding:0;list-style:none}.layers li{display:flex;justify-content:space-between;gap:18px;padding:8px 0;border-bottom:1px solid #2b2a26;font-size:13px}.layers small{color:#a7a49c;white-space:nowrap}.artwork{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:14px}.art{font-size:12px;color:#aeaba3;text-decoration:none}.art img{width:100%;height:110px;object-fit:contain;background:#d7d5ce;border-radius:4px;display:block;margin-bottom:6px}.advisories{padding-left:20px;font-size:13px;color:#b7aa90}footer{border-top:1px solid #36342e;padding-top:25px;padding-bottom:50px;font-size:14px;color:#aeaba3}footer a{color:#dbc18e}a:focus-visible,button:focus-visible,summary:focus-visible{outline:2px solid #d3ad69;outline-offset:4px}@media(max-width:900px){header,main,footer{padding-left:22px;padding-right:22px}header{display:block}header p{margin-top:20px;max-width:60ch}article{grid-template-columns:1fr;gap:10px}.description{padding:0 8px}.visual{max-width:740px}h2{font-size:57px}.route{margin-top:6px}article+article{padding-top:30px}}@media(prefers-reduced-motion:reduce){.ring{transition:none}}@media print{body{background:white;color:#222}header,main,footer{padding:12px}h2{font-size:36px}article{grid-template-columns:1fr 1fr;break-inside:avoid;padding:18px 0}nav,details,.caption{display:none}.description p,.subtitle,.route,dt,.finding{color:#333}.downloads a{color:#333}}
</style><header><h1>Solstice &amp; Nocturne</h1><p>Two signets built in RingDesigner.<br>18.200 mm nominal bore. Editable artwork and manufacturing files included.</p></header><main>'''+"".join(articles)+'''</main><footer><p>Open either <strong>design.ring.json</strong> in RingDesigner, or choose it from the <strong>masterwork-signets</strong> folder in the design library. The casting ZIP contains the compensated pattern; do not shrink-scale it again.</p><p><a href="contact-sheet.png">Comparison image</a> · <a href="contact-sheet.pdf">Printable comparison</a> · <a href="README.md">Build and manufacturing notes</a> · <a href="independent-checks.json">Independent file checks</a></p></footer><script>
document.querySelectorAll('button[data-view]').forEach(button=>button.addEventListener('click',()=>{const article=button.closest('article'),view=button.dataset.view,img=article.querySelector('.ring');article.querySelectorAll('button').forEach(b=>b.setAttribute('aria-pressed',String(b===button)));article.querySelector('.photo').classList.toggle('detail',view==='seal');img.src=article.dataset.ring+'/'+view+(view==='turntable'?'.gif':'.png');img.alt=article.querySelector('h2').textContent+', '+button.textContent.toLowerCase()+', actual model render';article.querySelector('.caption').textContent=view==='pattern'?'Compensated metal pattern: bench operations and stone references omitted.':view==='structure'?'The same signet with all decorative layers hidden.':view==='seal'?'Magnified seal: original geometry, shown with intended finish colours.':'Actual geometry with intended metal and stone colours. Select a view to inspect.';}));
</script></html>'''
(root / "index.html").write_text(page)
sheet.append('<text x="70" y="1180" font-family="sans-serif" font-size="19" fill="#aeaba3">Editable sources and manufacturing evidence accompany each design. Physical casting trials remain required.</text></svg>')
(root / "contact-sheet.svg").write_text("\n".join(sheet))
readme += ring_notes
readme += ["## Files and reproducibility", "", "- `design.ring.json`: nominal source; SVGs, generated patterns, brush strokes, text, masks and setting data travel with it.", "- `nominal.stl` / `nominal.3mf`: finished metal geometry. Stones are excluded.", "- `pattern-package/` and its ZIP: shrink-compensated casting mesh, complete source, recipe, report, mold/feeding diagram and process sheet. Do not compensate these files again.", "- `verification.json`: both saved sources rebuilt identical nominal and pattern triangles using empty alpha libraries.", "- `release-fine.json`: Solstice’s final 0.075 mm withdrawal screening.", "- `wall-screen.json`: additional sparse local-normal screening on a coarser mesh; inspect flagged small edges and details. The radial screen is not a guarantee of minimum thickness everywhere.", "- `artwork/`: original SVGs and lossless 16-bit alphas, including withdrawal stock and engraving masks.", "- `preview-metal.glb`: lighter metal-only 3D preview. Production meshes retain the full detail.", "- `turntable.gif`: actual metal and reference-stone geometry, rendered by the app.", "", "Rebuild in a new output directory:", "", "```sh", "RUSTUP_FORCE_ARG0=cargo /home/shadowbroker/.cargo/bin/cargo run -p ringdesign-core --example masterwork_signets -- NEW_DIRECTORY", "python tools/catalog_masterwork_signets.py NEW_DIRECTORY", "rsvg-convert -o NEW_DIRECTORY/contact-sheet.png NEW_DIRECTORY/contact-sheet.svg", "rsvg-convert -f pdf -o NEW_DIRECTORY/contact-sheet.pdf NEW_DIRECTORY/contact-sheet.svg", "```", "", "Use `--draft` for faster previews, `--sand` or `--wax` to author one design. The generator never overwrites an existing output directory. App-derived alphas are quantized once to their saved 16-bit representation before meshing, so saved files rebuild exactly.", "", "Creating these rings exposed a stone-preview bug: relief changed the arc-length walk and tilted gems onto pocket walls. Stone previews now share the setting report’s coordinate frames; the new asymmetric-relief regression and four existing gem tests pass. The desktop binary was rebuilt.", ""]
(root / "README.md").write_text("\n".join(readme) + "\nRecheck existing geometry with `cargo run -p ringdesign-core --example masterwork_signets -- COLLECTION --verify`.\n")
print(root / "index.html")
