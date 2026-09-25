"""Validate collection manifests, stone coverage, and Reptilia sheet compatibility."""
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import zipfile

from PIL import Image

from catalog_collection import catalog
from render_collection import load_manifest, local_path, stone_specs

REPO = Path(__file__).resolve().parents[1]


class CollectionTools(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="ringdesign-collection-")
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def write_manifest(self, **changes):
        doc = {"collection": "probe", "title": "Probe", "rings": [{"slug": "ring", "title": "Ring", "exposed_controls": []}]}
        doc.update(changes)
        (self.root / "collection.json").write_text(json.dumps(doc))
        return doc

    def test_empty_collection_and_duplicate_or_unsafe_slugs_are_rejected(self):
        for rings in [[], [{"slug": "../outside"}], [{"slug": "same"}, {"slug": "same"}]]:
            self.write_manifest(rings=rings)
            with self.assertRaises(ValueError):
                load_manifest(self.root, "probe")

    def test_camera_views_require_unique_names_and_finite_positions(self):
        for views in [[], [{"name": "studio", "position": [0, 1]}], [{"name": "studio", "position": [0, 1, float("nan")]}], [{"name": "studio", "position": [0, 1, 2]}] * 2]:
            self.write_manifest(views=views)
            with self.assertRaises(ValueError):
                load_manifest(self.root, "probe")

    def test_local_asset_cannot_escape_through_parent_or_symlink(self):
        with self.assertRaises(ValueError):
            local_path(self.root, "../elsewhere.stl")
        (self.root / "escape").symlink_to(self.root.parent, target_is_directory=True)
        with self.assertRaises(ValueError):
            local_path(self.root, "escape/elsewhere.stl")

    def test_stones_keep_individual_tints_and_refuse_omitted_or_invalid_materials(self):
        for name in ("ruby", "sapphire"):
            (self.root / f"reference-{name}.stl").write_bytes(b"fixture")
        stones = [{"mesh": "reference-ruby.stl", "tint": [0.45, 0.01, 0.04], "ior": 1.76, "dispersion": 0.018}, {"mesh": "reference-sapphire.stl", "tint": [0.02, 0.06, 0.45], "ior": 1.77}]
        (self.root / "stones.json").write_text(json.dumps({"stones": stones}))
        got = stone_specs(self.root)
        self.assertEqual(got[0]["tint"], stones[0]["tint"])
        self.assertEqual(got[1]["ior"], 1.77)
        self.assertEqual(got[0]["dispersion"], 0.018)
        (self.root / "stones.json").write_text(json.dumps(stones[:1]))
        with self.assertRaisesRegex(ValueError, "omits"):
            stone_specs(self.root)
        stones[1]["ior"] = float("inf")
        (self.root / "stones.json").write_text(json.dumps(stones))
        with self.assertRaisesRegex(ValueError, "ior"):
            stone_specs(self.root)

    def test_generic_gallery_escapes_copy_and_zip_excludes_stale_renders(self):
        self.write_manifest(rings=[{"slug": "ring", "title": "<Ring>", "description": "<script>bad</script>", "exposed_controls": []}], views=[{"name": "hero", "label": "Hero", "render": False}])
        (self.root / "ring").mkdir()
        Image.new("RGB", (16, 16), (70, 80, 90)).save(self.root / "ring/hero.png")
        (self.root / "renders").mkdir()
        (self.root / "renders/stale.png").write_bytes(b"stale")
        catalog(self.root, load_manifest(self.root), REPO)
        self.assertIn("&lt;Ring&gt;", (self.root / "index.html").read_text())
        self.assertNotIn("<script>bad", (self.root / "index.html").read_text())
        with zipfile.ZipFile(self.root / "Probe-final-renders.zip") as archive:
            self.assertFalse(any("stale" in name for name in archive.namelist()))
            self.assertEqual(len(archive.namelist()), 3)

    def test_reptilia_sheet_pixels_match_original_tool(self):
        before, after = self.root / "before", self.root / "after"
        before.mkdir()
        after.mkdir()
        for root in (before, after):
            for slug in ("ecdysis", "tessera", "lorica", "ophidian", "varanus"):
                (root / slug).mkdir()
                for name in ("studio.png", "studio-face.png", "palm.png"):
                    shutil.copy2(REPO / "showcase/reptilia" / slug / name, root / slug / name)
        subprocess.run([sys.executable, str(REPO / "tools/catalog_reptilia.py"), str(before)], check=True, capture_output=True, text=True)
        catalog(after, load_manifest(after, "reptilia"), REPO)
        self.assertEqual((before / "contact-sheet.svg").read_bytes(), (after / "contact-sheet.svg").read_bytes())
        with Image.open(before / "Reptilia-collection.png") as old, Image.open(after / "Reptilia-collection.png") as new:
            self.assertEqual(old.size, new.size)
            self.assertEqual(old.tobytes(), new.tobytes())


if __name__ == "__main__":
    unittest.main()
