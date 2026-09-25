"""Render exported collection meshes with manifest cameras and stone materials."""
from pathlib import Path
import argparse
import json
import math
import re
import sys

REPTILIA_ITEMS = [
    ("ecdysis", "Ecdysis", "Ventral scales", "8.5 mm band", "Broad, bowed belly plates meet rows of fine keeled scales."),
    ("tessera", "Tessera", "Shield mosaic", "9 mm band", "Hexagonal shields merge into broad chevrons around the band."),
    ("lorica", "Lorica", "Crocodile armour", "8 mm band", "Rectangular osteoderms carry low dorsal keels and a fine grain."),
    ("ophidian", "Ophidian", "Amethyst serpent", "Factory signet 013 · 7 × 5 mm amethyst", "A purple oval sits within a continuous serpent skin, from the face to the palm."),
    ("varanus", "Varanus", "Sovereign scales", "Factory signet 017 · patterned face", "A broad shield crown grades into smaller scales along the shoulders and cheeks."),
]
DEFAULT_VIEWS = [
    {"name": "studio", "label": "Portrait", "position": [26, 39, 36]},
    {"name": "studio-face", "label": "Close-up", "position": [2, 52, 14]},
    {"name": "palm", "label": "Palm", "position": [26, -39, 36]},
]


def safe_name(value):
    if not isinstance(value, str) or not re.fullmatch(r"[a-z0-9]+(?:[-_][a-z0-9]+)*", value):
        raise ValueError(f"Invalid slug or view name: {value!r}")
    return value


def local_path(root, value):
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or not path.parts:
        raise ValueError(f"Expected a relative file below {root}: {value!r}")
    result = (root / path).resolve()
    if not result.is_relative_to(root.resolve()):
        raise ValueError(f"File escapes {root}: {value!r}")
    return result


def finite_vector(value, size, label):
    if not isinstance(value, list) or len(value) != size or any(
        isinstance(x, bool) or not isinstance(x, (int, float)) or not math.isfinite(x) for x in value
    ):
        raise ValueError(f"{label} must contain {size} finite numbers")
    return value


def validate_views(views):
    if not isinstance(views, list) or not 1 <= len(views) <= 32:
        raise ValueError("Each render set must list 1–32 views")
    names = set()
    for view in views:
        if not isinstance(view, dict):
            raise ValueError("Each view must be an object")
        name = safe_name(view.get("name"))
        if name in names:
            raise ValueError(f"Duplicate view: {name}")
        names.add(name)
        if view.get("render", True):
            finite_vector(view.get("position"), 3, "camera position")
        if not isinstance(view.get("label", name), str):
            raise ValueError("View labels must be text")


def load_manifest(root, collection=None):
    path = root / "collection.json"
    if path.exists():
        manifest = json.loads(path.read_text())
    elif collection == "reptilia" or (collection is None and root.name == "reptilia"):
        manifest = {
            "collection": "reptilia", "title": "Reptilia", "metal": "silver", "layout": "reptilia",
            "subtitle": "THREE BANDS · TWO SIGNETS · ONE AMETHYST",
            "description": "Five studies in scale, shield and skin.\nThree bands. Two signets. One amethyst.",
            "finish": "Polished silver · darkened recesses · sculpted surfaces",
            "sheet_note": "Rendered from actual ring geometry. Fine detail and stone setting completed at the bench.",
            "views": [dict(v, render=v["name"] != "palm") for v in DEFAULT_VIEWS],
            "rings": [dict(zip(("slug", "title", "subtitle", "spec", "description"), row)) for row in REPTILIA_ITEMS],
        }
    else:
        raise ValueError(f"Missing collection manifest: {path}")
    if not isinstance(manifest, dict):
        raise ValueError("collection.json must contain an object")
    ident = safe_name(manifest.get("collection", collection or root.name))
    if collection is not None and ident != collection:
        raise ValueError(f"Requested {collection!r}, manifest describes {ident!r}")
    manifest["collection"] = ident
    manifest.setdefault("title", ident.title())
    if not isinstance(manifest["title"], str) or not manifest["title"].strip():
        raise ValueError("Collection title must be nonempty text")
    if any(c in manifest["title"] for c in "/\\\0") or manifest["title"] in (".", ".."):
        raise ValueError("Collection title must also be a safe output filename")
    rings = manifest.get("rings")
    if not isinstance(rings, list) or not 1 <= len(rings) <= 100:
        raise ValueError("A collection must list 1–100 rings")
    slugs = set()
    validate_views(manifest.get("views", DEFAULT_VIEWS))
    for ring in rings:
        if not isinstance(ring, dict):
            raise ValueError("Each ring must be an object")
        slug = safe_name(ring.get("slug"))
        if slug in slugs:
            raise ValueError(f"Duplicate ring: {slug}")
        slugs.add(slug)
        ring.setdefault("title", slug.replace("-", " ").title())
        for field in ("title", "subtitle", "spec", "description"):
            ring.setdefault(field, "")
            if not isinstance(ring[field], str):
                raise ValueError(f"{slug}: {field} must be text")
        finite_vector(ring.get("centre", [0, 1.4 if ident == "reptilia" and slug in ("ophidian", "varanus") else 0, 0]), 3, "centre")
        scale = ring.get("ortho_scale", 30.5)
        if isinstance(scale, bool) or not isinstance(scale, (int, float)) or not math.isfinite(scale) or scale <= 0:
            raise ValueError("Orthographic scale must be positive and finite")
        validate_views(ring.get("views", manifest.get("views", DEFAULT_VIEWS)))
        if "sheet_view" in ring:
            safe_name(ring["sheet_view"])
    manifest.setdefault("views", DEFAULT_VIEWS)
    manifest.setdefault("metal", "gold")
    if manifest["metal"] not in ("gold", "silver"):
        raise ValueError("Metal must be gold or silver")
    return manifest


def stone_specs(root, legacy=False):
    path = root / "stones.json"
    if path.exists():
        data = json.loads(path.read_text())
        specs = data.get("stones") if isinstance(data, dict) else data
    elif legacy and (root / "reference-amethyst.stl").exists():
        specs = [{"mesh": "reference-amethyst.stl", "name": "Amethyst / violet quartz", "tint": [0.22, 0.012, 0.40], "ior": 1.55, "transmission": 0.72}]
    elif list(root.glob("reference-*.stl")):
        raise ValueError(f"{root}: reference stone meshes need stones.json material entries")
    else:
        specs = []
    if not isinstance(specs, list) or len(specs) > 10000:
        raise ValueError("stones.json must list at most 10000 stone meshes")
    seen = set()
    for spec in specs:
        if not isinstance(spec, dict) or not isinstance(spec.get("mesh"), str):
            raise ValueError("Each stone entry needs a mesh path")
        mesh = local_path(root, spec["mesh"])
        if mesh in seen or mesh.suffix.lower() != ".stl" or not mesh.is_file():
            raise ValueError(f"Missing, repeated, or non-STL stone mesh: {mesh}")
        seen.add(mesh)
        tint = finite_vector(spec.get("tint"), 3, "stone tint")
        if any(x < 0 or x > 1 for x in tint):
            raise ValueError("Stone tint is linear RGB in 0..1")
        for key, default, low, high in [("ior", 1.55, 1, 4), ("dispersion", 0, 0, 0.3), ("roughness", 0.065, 0, 1), ("transmission", 0.72, 0, 1)]:
            value = spec.setdefault(key, default)
            if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or not low <= value <= high:
                raise ValueError(f"Invalid stone {key}: {value!r}")
        if spec["ior"] - spec["dispersion"] / 2 < 1:
            raise ValueError("Dispersed IOR must stay at least 1")
    missing = {p.resolve() for p in root.glob("reference-*.stl")} - seen
    if missing:
        raise ValueError(f"Stone manifest omits reference meshes: {sorted(map(str, missing))}")
    return specs


def render(args, manifest):
    import bpy
    from mathutils import Quaternion, Vector

    def aim(obj, point):
        obj.rotation_euler = (Vector(point) - obj.location).to_track_quat("-Z", "Y").to_euler()

    def metal():
        silver = manifest["metal"] == "silver"
        m = bpy.data.materials.new("Oxidized sterling / polished relief" if silver else "Studio gold / darkened recesses")
        if m.node_tree is None:
            m.use_nodes = True
        n, links = m.node_tree.nodes, m.node_tree.links
        p = n.get("Principled BSDF")
        p.inputs["Metallic"].default_value = 1.0
        p.inputs["Roughness"].default_value = 0.23
        ao = n.new("ShaderNodeAmbientOcclusion")
        ao.inputs["Distance"].default_value = 0.50
        ao.samples = 16
        ramp = n.new("ShaderNodeValToRGB")
        ramp.color_ramp.elements[0].position = 0.55
        ramp.color_ramp.elements[0].color = (0.014, 0.017, 0.019, 1) if silver else (0.018, 0.012, 0.004, 1)
        ramp.color_ramp.elements[1].position = 0.97
        ramp.color_ramp.elements[1].color = (0.76, 0.79, 0.83, 1) if silver else (0.83, 0.56, 0.20, 1)
        mid = ramp.color_ramp.elements.new(0.84)
        mid.color = (0.12, 0.14, 0.16, 1) if silver else (0.15, 0.075, 0.018, 1)
        links.new(ao.outputs["AO"], ramp.inputs["Fac"])
        links.new(ramp.outputs["Color"], p.inputs["Base Color"])
        return m

    def stone(spec):
        m = bpy.data.materials.new(spec.get("name", Path(spec["mesh"]).stem))
        if m.node_tree is None:
            m.use_nodes = True
        n, links = m.node_tree.nodes, m.node_tree.links
        p = n.get("Principled BSDF")
        p.inputs["Base Color"].default_value = (*spec["tint"], 1)
        p.inputs["Roughness"].default_value = spec["roughness"]
        p.inputs["IOR"].default_value = spec["ior"]
        p.inputs["Transmission Weight"].default_value = spec["transmission"]
        if spec["dispersion"] > 0:
            channels = []
            for index, offset in enumerate((-0.5, 0, 0.5)):
                glass = n.new("ShaderNodeBsdfGlass")
                colour = [0, 0, 0, 1]
                colour[index] = spec["tint"][index]
                glass.inputs["Color"].default_value = colour
                glass.inputs["IOR"].default_value = spec["ior"] + offset * spec["dispersion"]
                glass.inputs["Roughness"].default_value = spec["roughness"]
                channels.append(glass.outputs[0])
            add = n.new("ShaderNodeAddShader")
            links.new(channels[0], add.inputs[0])
            links.new(channels[1], add.inputs[1])
            total = n.new("ShaderNodeAddShader")
            links.new(add.outputs[0], total.inputs[0])
            links.new(channels[2], total.inputs[1])
            p.inputs["Transmission Weight"].default_value = 0
            mix = n.new("ShaderNodeMixShader")
            mix.inputs[0].default_value = spec["transmission"]
            links.new(p.outputs[0], mix.inputs[1])
            links.new(total.outputs[0], mix.inputs[2])
            links.new(mix.outputs[0], n.get("Material Output").inputs["Surface"])
        return m

    def load(path, material, smooth):
        bpy.ops.wm.stl_import(filepath=str(path.resolve()), forward_axis="Y", up_axis="Z")
        obj = bpy.context.object
        obj.data.materials.append(material)
        for poly in obj.data.polygons:
            poly.use_smooth = smooth

    selected = [r for r in manifest["rings"] if args.slug is None or r["slug"] == args.slug]
    if not selected:
        raise ValueError(f"Unknown ring: {args.slug}")
    for item in selected:
        slug = item["slug"]
        root = args.source / slug
        specs = stone_specs(root, manifest["collection"] == "reptilia")
        if not (root / "finished-metal.stl").is_file():
            raise ValueError(f"Missing finished mesh: {root}")
        bpy.ops.object.select_all(action="SELECT")
        bpy.ops.object.delete(use_global=False)
        for mesh in list(bpy.data.meshes):
            if mesh.users == 0:
                bpy.data.meshes.remove(mesh)
        for material in list(bpy.data.materials):
            if material.users == 0:
                bpy.data.materials.remove(material)
        scene = bpy.context.scene
        scene.render.engine = "CYCLES"
        prefs = bpy.context.preferences.addons["cycles"].preferences
        prefs.compute_device_type = args.device if args.device != "CPU" else "NONE"
        prefs.get_devices()
        for device in prefs.devices:
            device.use = device.type == args.device
        scene.cycles.device = "CPU" if args.device == "CPU" else "GPU"
        scene.cycles.samples = 48 if args.draft else 160
        scene.cycles.use_denoising = True
        scene.cycles.max_bounces = 10
        scene.render.resolution_x = scene.render.resolution_y = args.edge or (1050 if args.draft else 1800)
        scene.render.resolution_percentage = 100
        scene.render.image_settings.file_format = "PNG"
        if scene.world.node_tree is None:
            scene.world.use_nodes = True
        scene.world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.065, 0.07, 0.08, 1)
        scene.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.32
        scene.render.film_transparent = True
        scene.view_settings.view_transform = "AgX"
        load(root / "finished-metal.stl", metal(), True)
        for spec in specs:
            load(local_path(root, spec["mesh"]), stone(spec), False)
        legacy_tall = manifest["collection"] == "reptilia" and slug in ("ophidian", "varanus")
        centre = item.get("centre", [0, 1.4 if legacy_tall else 0, 0])
        for name, position, energy, size, size_y, color in [
            ("Key", (-20, 28, 34), 75000, 24, 42, (0.91, 0.95, 1.0)),
            ("Strip", (28, 5, 14), 52000, 9, 35, (1.0, 0.96, 0.91)),
            ("Rim", (-5, 24, -23), 80000, 22, 10, (0.84, 0.91, 1.0)),
            ("Bore fill", (0, -25, 30), 20000, 28, 18, (0.9, 0.94, 1.0)),
        ]:
            data = bpy.data.lights.new(name, "AREA")
            data.energy, data.shape = energy, "RECTANGLE"
            data.size, data.size_y, data.color = size, size_y, color
            obj = bpy.data.objects.new(name, data)
            scene.collection.objects.link(obj)
            obj.location = position
            aim(obj, centre)
        data = bpy.data.cameras.new("Product camera")
        camera = bpy.data.objects.new("Product camera", data)
        scene.collection.objects.link(camera)
        scene.camera = camera
        data.type = "ORTHO"
        data.ortho_scale = item.get("ortho_scale", 33.5 if legacy_tall else 30.5)
        views = [v for v in item.get("views", manifest["views"]) if v.get("render", True)]
        for index, view in enumerate(views):
            target = root / (view["name"] + ".png")
            if (args.draft and index > 0) or (args.resume and target.exists()):
                continue
            camera.location = view["position"]
            aim(camera, centre)
            camera.rotation_euler = (camera.rotation_euler.to_quaternion() @ Quaternion((0, 0, 1), math.pi)).to_euler()
            scene.render.filepath = str(target.resolve())
            print(f"Rendering {slug} / {view['name']} on {args.device}", flush=True)
            bpy.ops.render.render(write_still=True)
        if not args.draft:
            bpy.ops.wm.save_as_mainfile(filepath=str((root / "studio.blend").resolve()))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, epilog="stones.json entries: mesh, name, linear RGB tint, ior, dispersion (RGB IOR spread), roughness, transmission.")
    parser.add_argument("source", type=Path)
    parser.add_argument("slug", nargs="?")
    parser.add_argument("--collection")
    parser.add_argument("--draft", action="store_true")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--device", choices=("CPU", "CUDA", "OPTIX"), default="CUDA")
    parser.add_argument("--edge", type=int, help="Override render edge for a material/camera proof")
    args = parser.parse_args(argv)
    if args.edge is not None and not 32 <= args.edge <= 8192:
        parser.error("--edge must be in 32..8192")
    render(args, load_manifest(args.source, args.collection))


if __name__ == "__main__":
    main(sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else None)
