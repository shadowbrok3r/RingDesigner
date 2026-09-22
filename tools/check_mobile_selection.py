#!/usr/bin/env python3
"""Selection regression explored with ARTEMIS on 2026-09-19.

Requires an authorized rooted emulator, RingDesigner open in its Model workspace,
and View > layout debugging enabled. The --feature point uses the same normalized
1000 x 1000 coordinates as the verified ARTEMIS path. Controls use live semantic
layout bounds first, with an explicit coordinate fallback only if unavailable.
No geometry or saved design is modified.
"""
import argparse
import json
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--serial', required=True)
parser.add_argument('--feature', nargs=2, type=float, default=[735, 430])
args = parser.parse_args()
adb = ['adb', '-s', args.serial]
path = '/data/user/0/com.kingsofalchemy.ringdesigner/files/ringdesigner/layout-debug.json'

def command(*parts):
    return subprocess.check_output(adb + list(parts), text=True, timeout=10)

def state():
    return json.loads(command('shell', 'su', '0', 'cat', path))

def wait_for(predicate, description, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        data = state()
        if predicate(data):
            return data
        time.sleep(0.12)
    raise AssertionError(f'Timed out: {description}; selection={data.get("selection")}')

size = command('shell', 'wm', 'size').strip().split()[-1]
width, height = map(int, size.split('x'))

def point(normalized):
    return [round(normalized[0] * width / 1000), round(normalized[1] * height / 1000)]

def tap_pixels(xy):
    command('shell', 'input', 'tap', *map(str, xy))

def tap_control(label, fallback):
    try:
        data = state()
        bounds = next(c['rect'] for c in data['controls'] if c['label'] == label)
        scale = data['pointer']['pixels_per_point']
        xy = [round((bounds[i] + bounds[i + 2]) * scale / 2) for i in range(2)]
    except (KeyError, StopIteration, ValueError):
        xy = point(fallback)
    tap_pixels(xy)

def cleared(data):
    selection = data['selection']
    return all(selection[k] is None for k in ['layer', 'node', 'stone']) and not selection['hit'] and not selection['handles']

initial = state()
assert initial['selection']['tab'] == 'Ring', 'Open the Model workspace first'
assert initial['mode'] == 'Shape', 'Open Shape mode first'
tap_pixels(point(args.feature))
wait_for(lambda d: d['selection']['hit'], 'feature becomes selected')
tap_control('viewport/Clear', [97, 746])
wait_for(cleared, 'Clear releases the feature, node, stone and dimension handles')
tap_pixels(point(args.feature))
wait_for(lambda d: d['selection']['hit'], 'feature can be selected again')
tap_pixels(point([200, 200]))
wait_for(cleared, 'empty-space tap clears selection')
before = state()['camera']
command('shell', 'input', 'swipe', *map(str, point([200, 200])), *map(str, point([400, 200])), '300')
final = wait_for(lambda d: abs(d['camera']['yaw'] - before['yaw']) > 0.05, 'free orbit after clearing')
assert not final['overflow'], final['overflow']
print(json.dumps({'passed': ['feature selection', 'Clear', 'reselection', 'empty-space clear', 'free orbit', 'no horizontal overflow'], 'selection': final['selection']}, indent=2))
