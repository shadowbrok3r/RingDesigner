"""Render the actual exported Reptilia meshes in Blender/Cycles.

blender -b --factory-startup -P tools/render_reptilia.py -- SOURCE [SLUG] [--draft]
No generated concept imagery, displacement modifiers, or geometry edits.
"""
from pathlib import Path
import argparse
import math
import sys

import bpy
from mathutils import Vector, Quaternion

parser = argparse.ArgumentParser()
parser.add_argument("source", type=Path)
parser.add_argument("slug", nargs="?")
parser.add_argument("--draft", action="store_true")
parser.add_argument("--resume", action="store_true", help="Keep completed images")
parser.add_argument("--device", choices=("CPU", "CUDA", "OPTIX"), default="CUDA")
args = parser.parse_args(sys.argv[sys.argv.index("--") + 1:])


def aim(obj, point):
    obj.rotation_euler = (Vector(point) - obj.location).to_track_quat("-Z", "Y").to_euler()


def silver():
    m = bpy.data.materials.new("Oxidized sterling / polished relief")
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
    ramp.color_ramp.elements[0].color = (0.014, 0.017, 0.019, 1)
    ramp.color_ramp.elements[1].position = 0.97
    ramp.color_ramp.elements[1].color = (0.76, 0.79, 0.83, 1)
    mid = ramp.color_ramp.elements.new(0.84)
    mid.color = (0.12, 0.14, 0.16, 1)
    links.new(ao.outputs["AO"], ramp.inputs["Fac"])
    links.new(ramp.outputs["Color"], p.inputs["Base Color"])
    return m


def amethyst():
    m = bpy.data.materials.new("Amethyst / violet quartz")
    m.use_nodes = True
    p = m.node_tree.nodes.get("Principled BSDF")
    p.inputs["Base Color"].default_value = (0.22, 0.012, 0.40, 1)
    p.inputs["Roughness"].default_value = 0.065
    p.inputs["IOR"].default_value = 1.55
    p.inputs["Transmission Weight"].default_value = 0.72
    return m


def load(path, material, smooth):
    bpy.ops.wm.stl_import(filepath=str(path.resolve()), forward_axis="Y", up_axis="Z")
    obj = bpy.context.object
    obj.data.materials.append(material)
    for poly in obj.data.polygons:
        poly.use_smooth = smooth
    return obj


def render(slug):
    root = args.source / slug
    bpy.ops.object.select_all(action="SELECT")
    bpy.ops.object.delete(use_global=False)
    scene = bpy.context.scene
    scene.render.engine = "CYCLES"
    prefs = bpy.context.preferences.addons["cycles"].preferences
    prefs.compute_device_type = args.device if args.device != "CPU" else "NONE"
    prefs.get_devices()
    for d in prefs.devices:
        d.use = d.type == args.device
    scene.cycles.device = "CPU" if args.device == "CPU" else "GPU"
    scene.cycles.samples = 48 if args.draft else 160
    scene.cycles.use_denoising = True
    scene.cycles.max_bounces = 10
    scene.render.resolution_x = scene.render.resolution_y = 1050 if args.draft else 1800
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = "PNG"
    scene.world.use_nodes = True
    scene.world.node_tree.nodes["Background"].inputs["Color"].default_value = (0.065, 0.07, 0.08, 1)
    scene.world.node_tree.nodes["Background"].inputs["Strength"].default_value = 0.32
    scene.render.film_transparent = True
    scene.view_settings.view_transform = "AgX"
    ring = load(root / "finished-metal.stl", silver(), True)
    if (root / "reference-amethyst.stl").exists():
        load(root / "reference-amethyst.stl", amethyst(), False)
    centre = (0, 1.4 if slug in ("ophidian", "varanus") else 0, 0)
    # The ring's finger axis is Z; head is +Y. Broad cards reflect in its crowns.
    for name, position, energy, size, size_y, color in [
        ("Key", (-20, 28, 34), 75000, 24, 42, (0.91, 0.95, 1.0)),
        ("Strip", (28, 5, 14), 52000, 9, 35, (1.0, 0.96, 0.91)),
        ("Rim", (-5, 24, -23), 80000, 22, 10, (0.84, 0.91, 1.0)),
        ("Bore fill", (0, -25, 30), 20000, 28, 18, (0.9, 0.94, 1.0)),
    ]:
        data = bpy.data.lights.new(name, "AREA")
        data.energy = energy
        data.shape = "RECTANGLE"
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
    data.ortho_scale = 33.5 if slug in ("ophidian", "varanus") else 30.5
    for view, position in [("studio", (26, 39, 36)), ("studio-face", (2, 52, 14))]:
        if args.draft and view != "studio":
            continue
        if args.resume and (root / f"{view}.png").exists():
            continue
        print(f"Rendering {slug} / {view} on {args.device}", flush=True)
        camera.location = position
        aim(camera, centre)
        camera.rotation_euler = (camera.rotation_euler.to_quaternion() @ Quaternion((0, 0, 1), math.pi)).to_euler()
        scene.render.filepath = str((root / f"{view}.png").resolve())
        bpy.ops.render.render(write_still=True)
    # Preserve the scene for a repeatable lighting/material review.
    if not args.draft:
        bpy.ops.wm.save_as_mainfile(filepath=str((root / "studio.blend").resolve()))


for slug in ([args.slug] if args.slug else ["ecdysis", "tessera", "lorica", "ophidian", "varanus"]):
    render(slug)
