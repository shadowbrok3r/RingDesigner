"""Assemble the Reptilia review gallery and render delivery from actual mesh renders.

Design: charcoal and warm ivory, restrained amethyst accents, Garamond display
type with a compact sans serif index. Large uninterrupted images carry the page;
the collection sheet keeps every ring at a comparable visual scale.
"""
import argparse
import hashlib
import html
import json
import shutil
import subprocess
import zipfile
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("source", type=Path)
args = parser.parse_args()
root = args.source
repo = Path(__file__).resolve().parents[1]
items = [
    ("ecdysis", "Ecdysis", "Ventral scales", "8.5 mm band", "Broad, bowed belly plates meet rows of fine keeled scales."),
    ("tessera", "Tessera", "Shield mosaic", "9 mm band", "Hexagonal shields merge into broad chevrons around the band."),
    ("lorica", "Lorica", "Crocodile armour", "8 mm band", "Rectangular osteoderms carry low dorsal keels and a fine grain."),
    ("ophidian", "Ophidian", "Amethyst serpent", "Factory signet 013 · 7 × 5 mm amethyst", "A purple oval sits within a continuous serpent skin, from the face to the palm."),
    ("varanus", "Varanus", "Sovereign scales", "Factory signet 017 · patterned face", "A broad shield crown grades into smaller scales along the shoulders and cheeks."),
]
views = [("studio", "Portrait"), ("studio-face", "Close-up"), ("palm", "Palm")]
for slug, *_ in items:
    for view, _ in views:
        assert (root / slug / f"{view}.png").exists(), (slug, view)

fonts = repo / "showcase/masterwork-signets"
for name in ["EBGaramond.ttf", "FONT-LICENSE.txt"]:
    shutil.copy2(fonts / name, root / name)

def escape(s):
    return html.escape(s, quote=True)

sections = []
for i, (slug, name, subtitle, spec, description) in enumerate(items, 1):
    options = "".join(f'<button type="button" data-view="{key}" aria-pressed="{str(j == 0).lower()}">{label}</button>' for j, (key, label) in enumerate(views))
    sections.append(f'''<article id="{slug}" data-slug="{slug}">
      <div class="object"><button class="enlarge" aria-label="Enlarge {name}"><img src="{slug}/studio.png" alt="{name}, {subtitle.lower()}, in polished silver" loading="lazy" width="1800" height="1800"></button>
      <div class="views" aria-label="Views of {name}">{options}</div></div>
      <div class="caption"><span class="number">0{i} / REPTILIA</span><h2>{name}</h2><p class="subtitle">{subtitle}</p><p>{description}</p><p class="spec">{spec}</p>
      <div class="downloads"><a href="{slug}/studio.png" download="Reptilia-{name}.png">Download render ↗</a><a href="{slug}/editable-graph.ring.json" download>Openable design + graph ↗</a></div></div>
    </article>''')

page = '''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Reptilia — five rings</title><style>
@font-face{font-family:Garamond;src:url(EBGaramond.ttf) format('truetype');font-display:swap}
:root{--bg:#101313;--ink:#eeeae1;--muted:#a6aaa5;--line:#333936;--accent:#c3a7d0;--gap:clamp(24px,5vw,80px)}
*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--ink);font:16px/1.6 system-ui,sans-serif}button,a{-webkit-tap-highlight-color:transparent}a{color:inherit;text-underline-offset:5px}button{font:inherit;color:inherit}button:focus-visible,a:focus-visible{outline:2px solid var(--accent);outline-offset:5px}
header,main,footer{max-width:1600px;margin:auto;padding:0 var(--gap)}header{padding-top:65px;padding-bottom:50px}.kicker,.number{font-size:11px;letter-spacing:.19em;text-transform:uppercase;color:var(--muted)}.mast{display:flex;align-items:baseline;justify-content:space-between;border-bottom:1px solid var(--line);padding-bottom:14px}h1{font:normal clamp(88px,13vw,190px)/1 Garamond,serif;letter-spacing:-.035em;margin:44px 0 16px}header p{max-width:570px;color:var(--muted);margin:0}.intro-row{display:flex;justify-content:space-between;gap:30px;align-items:end}nav{display:flex;gap:22px;flex-wrap:wrap;font-size:12px}nav a{text-decoration:none}nav a:hover{color:var(--accent)}
article{display:grid;grid-template-columns:minmax(0,1.5fr) minmax(250px,1fr);gap:var(--gap);align-items:center;padding:40px 0 60px;border-top:1px solid var(--line);scroll-margin-top:24px}.object{min-width:0}.enlarge{display:block;width:100%;border:0;padding:0;background:transparent;cursor:zoom-in}.enlarge img{display:block;width:100%;height:auto;aspect-ratio:1;object-fit:contain}.views{display:flex;justify-content:center;gap:22px}.views button{border:0;border-bottom:1px solid transparent;background:transparent;padding:8px 0;font-size:12px;color:var(--muted);cursor:pointer}.views button[aria-pressed=true]{color:var(--ink);border-color:var(--accent)}.caption{max-width:420px}h2{font:normal clamp(50px,6vw,94px)/1.05 Garamond,serif;letter-spacing:-.025em;margin:20px 0 8px}.subtitle{font:italic 26px/1.3 Garamond,serif;color:var(--accent);margin:0 0 24px}.caption>p:not(.subtitle){color:var(--muted)}.spec{font-size:12px;margin-top:28px}.downloads{display:flex;flex-direction:column;gap:12px;font-size:12px;margin-top:25px}.downloads a{width:fit-content;text-decoration-color:var(--line)}.downloads a:hover{text-decoration-color:var(--accent)}footer{padding-top:44px;padding-bottom:65px;border-top:1px solid var(--line);font-size:12px;color:var(--muted)}footer p{max-width:780px}dialog{border:1px solid var(--line);padding:0;background:var(--bg);color:var(--ink);max-width:96vw;max-height:96vh}dialog::backdrop{background:#000d}dialog img{display:block;max-width:90vw;max-height:88vh;object-fit:contain}dialog button{position:absolute;right:12px;top:12px;background:var(--bg);border:1px solid var(--line);padding:6px 15px;cursor:pointer}
@media(max-width:700px){header{padding-top:30px;padding-bottom:30px}.mast{font-size:10px}.intro-row{display:block}nav{margin-top:25px;gap:14px}article{grid-template-columns:1fr;gap:30px;padding:20px 0 46px}.caption{max-width:none}h2{margin-top:10px}.number{font-size:10px}.views{gap:28px}.caption>p{max-width:470px}.downloads{gap:16px}footer{padding-bottom:40px}}
</style><header><div class="mast kicker"><span>Kings of Alchemy</span><span>Collection / 2026</span></div><h1>Reptilia</h1><div class="intro-row"><p>Five studies in scale, shield and skin.<br>Three bands. Two signets. One amethyst.</p><nav aria-label="Collection">NAVIGATION</nav></div></header><main>SECTIONS</main>
<footer><a href="Reptilia-collection.png" download>Collection sheet ↗</a><p>Rendered from the finished ring meshes. Silver polish, darkened recesses and amethyst colour describe the intended finish. The editable designs include separate casting stock and subtractive finishing layers.</p><p>All five meshes are closed. Withdrawal screening found no obstructions at 0.100 and 0.075 mm. Low-draft and fine-detail findings still require workshop review; none of these designs has been physically cast.</p></footer>
<dialog><button type="button" aria-label="Close enlarged render">Close ×</button><img alt=""></dialog><script>
const modal=document.querySelector('dialog'),large=modal.querySelector('img');
document.querySelectorAll('article').forEach(article=>{const img=article.querySelector('img');article.querySelectorAll('[data-view]').forEach(button=>button.addEventListener('click',()=>{article.querySelectorAll('[data-view]').forEach(b=>b.setAttribute('aria-pressed',String(b===button)));img.src=article.dataset.slug+'/'+button.dataset.view+'.png';img.alt=article.querySelector('h2').textContent+' — '+button.textContent;}));article.querySelector('.enlarge').addEventListener('click',()=>{large.src=img.src;large.alt=img.alt;modal.showModal();});});modal.querySelector('button').addEventListener('click',()=>modal.close());modal.addEventListener('click',e=>{if(e.target===modal)modal.close();});
</script></html>'''
page = page.replace("NAVIGATION", "".join(f'<a href="#{slug}">{name}</a>' for slug, name, *_ in items))
(root / "index.html").write_text(page.replace("SECTIONS", "\n".join(sections)))

svg = ['<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="2400" height="2920" viewBox="0 0 2400 2920">', '<rect width="2400" height="2920" fill="#101313"/>']
def text(x, y, content, size=32, colour="#eeeae1", family="sans-serif", extra=""):
    svg.append(f'<text x="{x}" y="{y}" fill="{colour}" font-size="{size}" font-family="{family}" {extra}>{escape(content)}</text>')

text(120, 130, "KINGS OF ALCHEMY", 28, "#a6aaa5", extra='letter-spacing="7"')
text(120, 345, "Reptilia", 225, family="EB Garamond, serif")
text(123, 422, "THREE BANDS · TWO SIGNETS · ONE AMETHYST", 28, "#a6aaa5", extra='letter-spacing="4"')
svg.append('<path d="M120 485H2280 M120 1450H2280 M120 2700H2280" stroke="#333936" stroke-width="2"/>')
for i, (slug, name, subtitle, spec, description) in enumerate(items):
    if i < 3:
        x, y, w = 80 + i * 760, 530, 720
        tx, ty = x + 40, 1300
        size = 74
    else:
        x, y, w = 150 + (i - 3) * 1100, 1530, 1000
        tx, ty = x + 50, 2540
        size = 86
    svg.append(f'<image x="{x}" y="{y}" width="{w}" height="{w}" xlink:href="{slug}/studio.png"/>')
    text(tx, ty, name, size, family="EB Garamond, serif")
    text(tx, ty + 52, subtitle, 30, "#c3a7d0")
    text(tx, ty + 100, spec, 25, "#a6aaa5")
text(120, 2790, "Polished silver · darkened recesses · sculpted surfaces", 30, "#a6aaa5")
text(120, 2846, "Rendered from actual ring geometry. Fine detail and stone setting completed at the bench.", 26, "#a6aaa5")
svg.append("</svg>")
(root / "contact-sheet.svg").write_text("\n".join(svg))
subprocess.run(["rsvg-convert", "-o", str(root / "Reptilia-collection.png"), str(root / "contact-sheet.svg")], check=True)

manifest = []
delivery = root / "renders"
delivery.mkdir(exist_ok=True)
shutil.copy2(root / "Reptilia-collection.png", delivery / "Reptilia-collection.png")
for slug, name, *_ in items:
    for source, label in [("studio", "portrait"), ("studio-face", "close-up"), ("palm", "palm")]:
        dest = delivery / f"Reptilia-{name}-{label}.png"
        shutil.copy2(root / slug / f"{source}.png", dest)
        manifest.append({"file": dest.name, "sha256": hashlib.sha256(dest.read_bytes()).hexdigest(), "source_mesh": f"{slug}/finished-metal.stl"})
(delivery / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
with zipfile.ZipFile(root / "Reptilia-final-renders.zip", "w", zipfile.ZIP_DEFLATED) as z:
    for path in sorted(delivery.iterdir()):
        z.write(path, f"Reptilia/{path.name}")
print(f"Gallery, collection sheet and {len(manifest)} final views: {root}")
