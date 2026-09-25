"""Build a manifest-driven collection gallery, contact sheet, and render ZIP."""
import argparse
import hashlib
import html
import json
import math
import shutil
import subprocess
import zipfile
from pathlib import Path

from render_collection import load_manifest

PAGE = r"""<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
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
</script></html>"""


def escape(value):
    return html.escape(str(value), quote=True)


def catalog(root, manifest, repo):
    title = manifest["title"]
    items = manifest["rings"]
    legacy = manifest.get("layout") == "reptilia"
    if legacy and [r["slug"] for r in items] != ["ecdysis", "tessera", "lorica", "ophidian", "varanus"]:
        raise ValueError("Reptilia layout requires its five original rings in order")
    for item in items:
        for view in item.get("views", manifest["views"]):
            path = root / item["slug"] / (view["name"] + ".png")
            if not path.is_file():
                raise ValueError(f"Missing render: {path}")
    fonts = repo / "showcase/masterwork-signets"
    for name in ["EBGaramond.ttf", "FONT-LICENSE.txt"]:
        target = root / name
        if target.resolve() != (fonts / name).resolve():
            shutil.copy2(fonts / name, target)
    sections = []
    for i, item in enumerate(items, 1):
        slug, name = item["slug"], item["title"]
        views = item.get("views", manifest["views"])
        first = views[0]["name"]
        options = "".join(f'<button type="button" data-view="{v["name"]}" aria-pressed="{str(j == 0).lower()}">{escape(v.get("label", v["name"]))}</button>' for j, v in enumerate(views))
        design = root / slug / "editable-graph.ring.json"
        download = f'<a href="{slug}/editable-graph.ring.json" download>Openable design + graph ↗</a>' if design.is_file() else ""
        sections.append(f'''<article id="{slug}" data-slug="{slug}">
      <div class="object"><button class="enlarge" aria-label="Enlarge {escape(name)}"><img src="{slug}/{first}.png" alt="{escape(name)}, {escape(item['subtitle'].lower())}, in studio {manifest['metal']}" loading="lazy" width="1800" height="1800"></button>
      <div class="views" aria-label="Views of {escape(name)}">{options}</div></div>
      <div class="caption"><span class="number">{i:02} / {escape(title.upper())}</span><h2>{escape(name)}</h2><p class="subtitle">{escape(item['subtitle'])}</p><p>{escape(item['description'])}</p><p class="spec">{escape(item['spec'])}</p>
      <div class="downloads"><a href="{slug}/{first}.png" download="{escape(title)}-{slug}.png">Download render ↗</a>{download}</div></div>
    </article>''')
    page = PAGE.replace("<title>Reptilia — five rings</title>", f"<title>{escape(title)} — {len(items)} rings</title>")
    page = page.replace("<h1>Reptilia</h1>", f"<h1>{escape(title)}</h1>")
    page = page.replace("Five studies in scale, shield and skin.<br>Three bands. Two signets. One amethyst.", escape(manifest.get("description", manifest.get("subtitle", ""))).replace("\n", "<br>"))
    page = page.replace("Reptilia-collection.png", escape(title) + "-collection.png")
    if not legacy:
        start, end = page.index("<footer>"), page.index("</footer>")
        page = page[:start] + f'<footer><a href="{escape(title)}-collection.png" download>Collection sheet ↗</a><p>{escape(manifest.get("footer", "Rendered from exported finished-metal and reference-stone meshes. Casting reports and editable designs accompany each ring."))}</p>' + page[end:]
    page = page.replace("NAVIGATION", "".join(f'<a href="#{r["slug"]}">{escape(r["title"])}</a>' for r in items))
    (root / "index.html").write_text(page.replace("SECTIONS", "\n".join(sections)))
    height = 2920 if legacy else 650 + math.ceil(len(items) / min(4, len(items))) * 790 + 210
    svg = [f'<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="2400" height="{height}" viewBox="0 0 2400 {height}">', f'<rect width="2400" height="{height}" fill="#101313"/>']

    def text(x, y, content, size=32, colour="#eeeae1", family="sans-serif", extra=""):
        svg.append(f'<text x="{x}" y="{y}" fill="{colour}" font-size="{size}" font-family="{family}" {extra}>{escape(content)}</text>')

    text(120, 130, "KINGS OF ALCHEMY", 28, "#a6aaa5", extra='letter-spacing="7"')
    text(120, 345, title, 225, family="EB Garamond, serif")
    text(123, 422, manifest.get("subtitle", ""), 28, "#a6aaa5", extra='letter-spacing="4"')
    if legacy:
        svg.append('<path d="M120 485H2280 M120 1450H2280 M120 2700H2280" stroke="#333936" stroke-width="2"/>')
    else:
        svg.append(f'<path d="M120 485H2280 M120 {height - 210}H2280" stroke="#333936" stroke-width="2"/>')
    for i, item in enumerate(items):
        if legacy and i < 3:
            x, y, w = 80 + i * 760, 530, 720
            tx, ty, size = x + 40, 1300, 74
        elif legacy:
            x, y, w = 150 + (i - 3) * 1100, 1530, 1000
            tx, ty, size = x + 50, 2540, 86
        else:
            columns = min(4, len(items))
            pitch = 2160 // columns
            w = min(640, pitch - 20)
            x, y = 120 + (i % columns) * pitch, 540 + (i // columns) * 790
            tx, ty, size = x + 20, y + w + 55, 58
        view = item.get("sheet_view", item.get("views", manifest["views"])[0]["name"])
        if not (root / item["slug"] / (view + ".png")).is_file():
            raise ValueError(f"Missing sheet view: {item['slug']}/{view}")
        svg.append(f'<image x="{x}" y="{y}" width="{w}" height="{w}" xlink:href="{item["slug"]}/{escape(view)}.png"/>')
        text(tx, ty, item["title"], size, family="EB Garamond, serif")
        text(tx, ty + 52, item["subtitle"], 30, "#c3a7d0")
        text(tx, ty + 100, item["spec"], 25, "#a6aaa5")
    text(120, height - 130, manifest.get("finish", "Studio gold · darkened recesses · stones set"), 30, "#a6aaa5")
    text(120, height - 74, manifest.get("sheet_note", "Rendered from the collection's actual ring geometry."), 26, "#a6aaa5")
    svg.append("</svg>")
    (root / "contact-sheet.svg").write_text("\n".join(svg))
    sheet = root / (title + "-collection.png")
    subprocess.run(["rsvg-convert", "-o", str(sheet), str(root / "contact-sheet.svg")], check=True)
    delivery = root / "renders"
    delivery.mkdir(exist_ok=True)
    shutil.copy2(sheet, delivery / sheet.name)
    records, files = [], [delivery / sheet.name]
    for item in items:
        for view in item.get("views", manifest["views"]):
            label = view.get("label", view["name"]).lower().replace(" ", "-")
            if not all(c.isalnum() or c in "-_" for c in label):
                label = view["name"]
            name = item["title"] if legacy else item["slug"]
            dest = delivery / f"{title}-{name}-{label}.png"
            if dest in files:
                raise ValueError(f"Duplicate delivery filename: {dest.name}")
            shutil.copy2(root / item["slug"] / (view["name"] + ".png"), dest)
            files.append(dest)
            records.append({"file": dest.name, "sha256": hashlib.sha256(dest.read_bytes()).hexdigest(), "source_mesh": f"{item['slug']}/finished-metal.stl"})
    index = delivery / "manifest.json"
    index.write_text(json.dumps(records, indent=2) + "\n")
    files.append(index)
    with zipfile.ZipFile(root / (title + "-final-renders.zip"), "w", zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(files):
            info = zipfile.ZipInfo(f"{title}/{path.name}", date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, path.read_bytes())
    print(f"Gallery, collection sheet and {len(records)} final views: {root}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--collection")
    parser.add_argument("--title")
    args = parser.parse_args()
    manifest = load_manifest(args.source, args.collection)
    if args.title:
        if any(c in args.title for c in "/\\\0") or args.title in (".", ".."):
            parser.error("--title must be a safe filename")
        manifest["title"] = args.title
    catalog(args.source, manifest, Path(__file__).resolve().parents[1])


if __name__ == "__main__":
    main()
