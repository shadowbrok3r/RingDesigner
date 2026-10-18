#!/usr/bin/env python3
"""Exercise the compact UI on an explicitly selected, root-enabled emulator.

Paths and gestures were explored with ARTEMIS on the S26 Ultra AVD. Controls
are located by the app's live layout labels, not fixed screen coordinates.
The original preferences, current design and rotation are restored even on
failure. Requires an installed native-ABI APK; never uninstalls or clears data.

python3 scripts/check-compact-ui.py --serial emulator-5554 --output /tmp/ui-check
"""
import argparse
import json
from pathlib import Path
import subprocess
import time

PACKAGE = "com.kingsofalchemy.ringdesigner"
ROOT = f"/data/user/0/{PACKAGE}/files/ringdesigner"
ACTIVITY = f"{PACKAGE}/com.github.egui_mobile.EguiNativeActivity"


class Device:
    def __init__(self, serial, output):
        self.adb = ["adb", "-s", serial]
        self.output = output
        self.results = []

    def run(self, *args, data=None):
        return subprocess.run(self.adb + list(args), input=data, check=True,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout

    def read(self, name):
        return self.run("exec-out", "cat", f"{ROOT}/{name}")

    def write(self, name, data):
        self.run("shell", "tee", f"{ROOT}/{name}", data=data)

    def state(self):
        return json.loads(self.read("layout-debug.json"))

    def wait(self, description, predicate, timeout=30):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            try:
                state = self.state()
                if predicate(state):
                    return state
            except (json.JSONDecodeError, subprocess.CalledProcessError):
                pass  # A frame may currently be replacing the report.
            time.sleep(0.1)
        raise AssertionError(f"Timed out: {description}")

    @staticmethod
    def control(state, label):
        return next((c for c in state["controls"] if c["label"] == label), None)

    def point(self, label):
        state = self.wait(label, lambda s: self.control(s, label))
        # egui popup sizing and Android insets can settle over several frames.
        last, stable = None, 0
        for _ in range(30):
            state = self.state()
            control = self.control(state, label)
            if control and control == last:
                stable += 1
                if stable >= 3:
                    break
            else:
                last, stable = control, 0
            time.sleep(0.1)
        assert control and stable >= 3, f"Unstable control: {label}"
        r, c = control["rect"], control["clip"]
        visible = [max(r[0], c[0]), max(r[1], c[1]), min(r[2], c[2]), min(r[3], c[3])]
        assert visible[2] > visible[0] and visible[3] > visible[1], label
        scale = state["pointer"]["pixels_per_point"]
        return tuple(round((visible[i] + visible[i + 2]) / 2 * scale) for i in (0, 1))

    def tap(self, label):
        x, y = self.point(label)
        self.run("shell", "input", "tap", str(x), str(y))

    def resize(self, landscape=False):
        before = self.state()["workspace"]["inspector_fraction"]
        x, y = self.point("inspector/resize")
        delta = round(90 * self.state()["pointer"]["pixels_per_point"])
        self.run("shell", "input", "swipe", str(x), str(y),
                 str(x - delta if landscape else x), str(y if landscape else y - delta), "600")
        axis = int(landscape)
        return self.wait("panel grew", lambda s: s["workspace"]["inspector_fraction"][axis] > before[axis] + .05)

    def rotate(self, landscape):
        self.run("shell", "settings", "put", "system", "user_rotation", str(int(landscape)))
        def ready(s):
            r = s["safe_rect"]
            return (r[2] - r[0] > r[3] - r[1]) == landscape
        self.wait("rotation", ready)
        self.point("inspector/resize")

    def launch(self, old_pid=None):
        self.run("shell", "am", "start", "-W", "-n", ACTIVITY)
        pid = int(self.run("shell", "pidof", PACKAGE).strip())
        assert pid != old_pid, "Restart reused the old process"
        return self.wait("app ready", lambda s: s["process_id"] == pid and not s["mesh"]["building"], 60)

    def checkpoint(self, name):
        state = self.state()
        assert not state["overflow"], f"{name}: {state['overflow']}"
        (self.output / f"{name}.json").write_text(json.dumps(state, indent=2))
        (self.output / f"{name}.png").write_bytes(self.run("exec-out", "screencap", "-p"))
        self.results.append(name)
        print(f"PASS {name}", flush=True)

    def exercise(self):
        full_bottom = self.state()["safe_rect"][3]
        self.tap("header/File")
        self.tap("file/New design...")
        self.tap("templates/search")
        self.run("shell", "input", "text", "court")
        self.wait("search filtered", lambda s: self.control(s, "templates/Court band") and
                  not self.control(s, "templates/Braided band"))
        self.run("shell", "input", "keyevent", "4")
        self.wait("keyboard closed", lambda s: abs(s["safe_rect"][3] - full_bottom) < 2)
        self.checkpoint("template-search")
        self.tap("templates/Court band")
        self.wait("template loaded", lambda s: s["selection"]["tab"] == "Ring" and not s["mesh"]["building"])

        self.resize()
        portrait = self.state()["workspace"]["inspector_fraction"][0]
        self.checkpoint("portrait-resized")
        self.tap("header/Panel")
        self.wait("panel hidden", lambda s: s["selection"]["sheet"] == "None")
        self.tap("header/Panel")
        self.wait("panel reopened", lambda s: self.control(s, "inspector/resize"))
        assert abs(self.state()["workspace"]["inspector_fraction"][0] - portrait) < .001

        self.tap("value/Finger opening")
        self.wait("keyboard open", lambda s: s["safe_rect"][3] < full_bottom - 100)
        def field_visible(state):
            field = self.control(state, "field/Finger opening")
            return field and field["rect"][1] >= field["clip"][1] - 1 and field["rect"][3] <= field["clip"][3] + 1
        state = self.wait("focused field scrolled into view", field_visible)
        self.point("value/Finger opening")
        assert abs(state["workspace"]["inspector_fraction"][0] - portrait) < .001
        self.checkpoint("keyboard-field-visible")
        self.run("shell", "input", "keyevent", "4")
        self.wait("keyboard closed", lambda s: abs(s["safe_rect"][3] - full_bottom) < 2)

        self.rotate(True)
        self.resize(True)
        self.checkpoint("landscape-resized")
        fractions = self.state()["workspace"]["inspector_fraction"]
        self.rotate(False)
        assert self.state()["workspace"]["inspector_fraction"] == fractions
        old_pid = self.state()["process_id"]
        self.run("shell", "am", "force-stop", PACKAGE)
        self.launch(old_pid)
        assert self.state()["workspace"]["inspector_fraction"] == fractions
        self.checkpoint("sizes-persist-after-restart")

        x, y = self.point("inspector/resize")
        self.run("shell", f"input tap {x} {y}; input tap {x} {y}")
        self.wait("double tap reset", lambda s: abs(s["workspace"]["inspector_fraction"][0] - .32) < .001)
        self.checkpoint("panel-reset")

        self.tap("menu/Tools")
        self.point("tools/Theme")
        self.checkpoint("grouped-tools")
        self.tap("tools/Theme")
        self.wait("Theme opened", lambda s: s["selection"]["tab"] == "Theme")
        self.tap("mode/Shape")
        self.tap("menu/View")
        self.tap("view/Zoom")
        self.point("camera/Zoom in")
        zoom = self.state()["camera"]["zoom"]
        self.tap("camera/Zoom in")
        self.wait("zoom action", lambda s: s["camera"]["zoom"] > zoom)
        assert self.control(self.state(), "camera/Zoom in"), "View menu closed during adjustment"
        self.checkpoint("view-inline-zoom")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    device = Device(args.serial, args.output)
    assert device.run("shell", "getprop", "ro.kernel.qemu").strip() == b"1", "Use an emulator"
    assert device.run("shell", "id", "-u").strip() == b"0", "Enable adb root on the selected emulator first"
    # Private preferences can contain credentials: retain only in memory and
    # restore exact bytes. Never copy them into the published evidence directory.
    originals = {name: device.read(name) for name in ("prefs.json", "current.ring.json")}
    rotation = {key: device.run("shell", "settings", "get", "system", key).decode().strip()
                for key in ("accelerometer_rotation", "user_rotation")}
    try:
        process = device.run("shell", "pidof", PACKAGE).strip()
    except subprocess.CalledProcessError:
        process = b""  # An APK upgrade stops the previous process.
    old_pid = int(process) if process else None
    try:
        device.run("shell", "am", "force-stop", PACKAGE)
        prefs = json.loads(originals["prefs.json"])
        prefs.update(editor_debug_layout=True, editor_inspector=True, editor_mode=0)
        prefs["workspace"]["inspector_fraction"] = [.32, .36]
        device.write("prefs.json", json.dumps(prefs).encode())
        device.run("shell", "settings", "put", "system", "accelerometer_rotation", "0")
        device.run("shell", "settings", "put", "system", "user_rotation", "0")
        device.launch(old_pid)
        device.exercise()
        (args.output / "result.json").write_text(json.dumps({"passed": device.results,
            "serial": args.serial, "framework": "ADB with live egui layout labels",
            "exploration": "ARTEMIS S26 Ultra emulator"}, indent=2))
    finally:
        device.run("shell", "am", "force-stop", PACKAGE)
        for name, data in originals.items():
            device.write(name, data)
        for key, value in rotation.items():
            device.run("shell", "settings", "put", "system", key, value)
        device.run("shell", "am", "start", "-n", ACTIVITY)


if __name__ == "__main__":
    main()
