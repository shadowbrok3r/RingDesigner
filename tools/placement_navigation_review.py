#!/usr/bin/env python3
"""Replay the ARTEMIS/ADB-observed 0.18 placement workflow on an emulator.

Requires Aster Atelier open in RingDesigner, portrait orientation, and adb root.
Uses live widget bounds and camera/draft state, plus the verified ring-relative
contact point for this fixture. Restores the source and preferences even on failure.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial', required=True)
    parser.add_argument('--out', type=Path, default=Path('target/navigation-review/replay'))
    args = parser.parse_args()
    if not re.fullmatch(r'emulator-\d+', args.serial):
        parser.error('Use an emulator, not a personal phone')
    args.out.mkdir(parents=True, exist_ok=True)
    adb = ['adb', '-s', args.serial]
    package = 'com.kingsofalchemy.ringdesigner'
    root = f'/data/user/0/{package}'
    folder = root + '/files/ringdesigner'

    def run(*command):
        return subprocess.check_output(adb + list(command), timeout=30)

    def stop():
        run('shell', 'am', 'force-stop', package)

    def start():
        run('shell', 'am', 'start', '-W', '-n', package + '/com.github.egui_mobile.EguiNativeActivity')

    run('root')
    run('wait-for-device')
    stop()
    prefs = run('exec-out', 'cat', folder + '/prefs.json')
    original = run('exec-out', 'cat', folder + '/current.ring.json')
    if 'Aster Atelier' not in json.loads(original)['name']:
        start()
        parser.error('Open the Aster Atelier fixture before this replay')
    uid = run('shell', 'stat', '-c', '%u', root).decode().strip()

    def write(name, data):
        temporary = args.out / 'restore.tmp'
        temporary.write_bytes(data)
        run('push', str(temporary), folder + '/' + name)
        run('shell', 'chown', uid + ':' + uid, folder + '/' + name)
        temporary.unlink()

    def layout():
        return json.loads(run('exec-out', 'cat', folder + '/layout-debug.json'))

    def wait(predicate, timeout=15):
        end = time.monotonic() + timeout
        last = None
        while time.monotonic() < end:
            try:
                last = layout()
                if predicate(last):
                    return last
            except (ValueError, subprocess.CalledProcessError):
                pass
            time.sleep(.10)
        (args.out / 'timeout.json').write_text(json.dumps(last, indent=2))
        raise AssertionError('Timed out waiting for UI state; see timeout.json')

    def rect(label, data=None):
        data = data or layout()
        control = next(c for c in data['controls'] if c['label'] == label)
        r, c = control['rect'], control['clip']
        return [max(r[0], c[0]), max(r[1], c[1]), min(r[2], c[2]), min(r[3], c[3])]

    def pixel(point):
        scale = layout()['pointer']['pixels_per_point']
        return [str(round(v * scale)) for v in point]

    def tap(label):
        b = rect(label)
        assert b[2] > b[0] and b[3] > b[1], label + ' is clipped'
        run('shell', 'input', 'tap', *pixel([(b[0]+b[2])/2, (b[1]+b[3])/2]))
        # egui Areas perform a sizing pass before accepting their first input.
        time.sleep(.35)

    def drag(a, b):
        run('shell', 'input', 'swipe', *pixel(a), *pixel(b), '650')

    def shot(name):
        (args.out / (name + '.png')).write_bytes(run('exec-out', 'screencap', '-p'))
        (args.out / (name + '.json')).write_text(json.dumps(layout(), indent=2))

    def angle_delta(a, b):
        return abs((a-b+math.pi) % (2*math.pi)-math.pi)

    def no_loupe(data):
        return all(c['label'] != 'viewport/magnifier' for c in data['controls'])

    results = []
    contact_process = None
    try:
        config = json.loads(prefs)
        config.update(editor_debug_layout=True, editor_inspector=False, editor_mode=1, editor_guides=False)
        config['navigation'] = dict(locked=False, magnifier=True)
        config['workspace'].update(rail_position=None, rail_collapsed=False, palette_position=None)
        write('prefs.json', json.dumps(config).encode())
        start()
        pid = int(run('shell', 'pidof', package))
        d = wait(lambda d: d.get('process_id') == pid and not d['mesh']['building'], 90)
        assert 'visual' in d, 'Install the final 0.18 build with placement diagnostics'
        assert d['safe_rect'][3] > d['safe_rect'][2], 'Portrait orientation required'
        count = d['history']['layers']
        for tool in ('Stamp', 'Path'):
            tap('floating/' + tool)
            wait(lambda d: d['visual']['tool'] == tool)
            tap('navigator/Signet face')
            v = rect('viewport')
            w, h = v[2]-v[0], v[3]-v[1]
            empty = [v[0]+w*.55, v[1]+h*.24]
            contact = [(v[0]+v[2])/2+w*.03, (v[1]+v[3])/2-w*.06]
            before = layout()['camera']
            drag(empty, [empty[0]+w*.12, empty[1]+h*.035])
            after = wait(lambda d: angle_delta(d['camera']['yaw'], before['yaw']) > .05)
            assert after['history']['layers'] == count and after['visual']['path_points'] == 0
            results.append(tool + ': empty space orbits without editing')
            tap('navigator/Signet face')
            drag(empty, contact)
            wait(lambda d: not d['pointer']['down'])
            assert layout()['history']['layers'] == count and layout()['visual']['path_points'] == 0
            results.append(tool + ': crossing onto the mesh keeps navigation ownership')
            assert rect('viewport-palette')[2]-rect('viewport-palette')[0] <= 181
            assert not layout()['overflow']
            shot(tool.lower() + '-narrow-palette')
        tap('navigator/Signet face')
        face = layout()['camera']
        tap('navigator/Turn left')
        left = layout()['camera']
        assert abs(angle_delta(left['yaw'], face['yaw'])-math.pi/2) < .001
        tap('navigator/Turn right')
        assert angle_delta(layout()['camera']['yaw'], face['yaw']) < .001
        tap('navigator/Opposite')
        assert abs(angle_delta(layout()['camera']['yaw'], face['yaw'])-math.pi) < .001
        tap('navigator/Opposite')
        assert angle_delta(layout()['camera']['yaw'], face['yaw']) < .001
        results.append('Quarter turns and opposite-side round trip are exact')
        tap('navigator/Lock view')
        wait(lambda d: d['navigation']['locked'])
        before = layout()['camera']
        drag(empty, [empty[0]+w*.08, empty[1]+h*.02])
        after = wait(lambda d: d['camera']['pan'] != before['pan'])['camera']
        assert after['yaw'] == before['yaw'] and after['pitch'] == before['pitch']
        tap('navigator/Right shoulder')
        assert abs(angle_delta(layout()['camera']['yaw'], before['yaw'])-math.pi/2) < .001
        assert abs(layout()['camera']['pitch']) < .001
        assert layout()['navigation']['locked']
        results.append('Locked dragging pans; explicit view commands still work')
        tap('navigator/Signet face')
        tap('floating/Stamp')
        position = pixel(contact)
        for magnifier in (True, False):
            if not magnifier:
                tap('navigator/Magnifier')
                wait(lambda d: not d['navigation']['magnifier'])
            contact_process = subprocess.Popen(adb + ['shell', 'input', 'swipe', *position, *position, '3200'])
            active = wait(lambda d: d['pointer']['down'] and no_loupe(d) != magnifier, 2.5)
            assert active['history']['layers'] == count, 'Stamp committed before release'
            if magnifier:
                time.sleep(.8)  # Past Android's long-touch conversion.
                assert not no_loupe(layout())
                lens = rect('viewport/magnifier')
                assert lens[3] < contact[1]-35, 'Lens covers the contact'
                shot('stamp-live-magnifier')
            contact_process.wait(timeout=6)
            contact_process = None
            wait(lambda d: d['history']['layers'] == count+1)
            tap('header/Undo')
            wait(lambda d: d['history']['layers'] == count and no_loupe(d))
            results.append(('Visible' if magnifier else 'Hidden') + ' loupe: hold preserves preview, release places once, Undo restores')
        tap('navigator/Magnifier')
        tap('floating/Path')
        contact_process = subprocess.Popen(adb + ['shell', 'input', 'swipe', *position, *position, '2400'])
        wait(lambda d: d['pointer']['down'] and not no_loupe(d), 2)
        shot('path-live-magnifier')
        contact_process.wait(timeout=6)
        contact_process = None
        wait(lambda d: d['visual']['path_points'] == 1)
        assert layout()['history']['layers'] == count
        results.append('Held path placement adds one draft point on release, without applying a layer')
        tap('floating/Select')
        tap('viewport-palette/close')
        end = time.monotonic()+20
        while run('exec-out', 'cat', folder+'/current.ring.json') != original:
            if time.monotonic() > end:
                raise AssertionError('Undo did not restore the original design bytes')
            time.sleep(.2)
        results.append('Original design bytes preserved')
        report = dict(passed=results, sha256=hashlib.sha256(original).hexdigest())
        (args.out/'results.json').write_text(json.dumps(report, indent=2)+'\n')
        print(json.dumps(report, indent=2))
    finally:
        if contact_process is not None:
            contact_process.wait(timeout=10)
        stop()
        write('prefs.json', prefs)
        write('current.ring.json', original)
        start()


if __name__ == '__main__':
    main()
