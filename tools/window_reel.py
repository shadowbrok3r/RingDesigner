#!/usr/bin/env python3
"""Record real RingDesigner interactions on an isolated X11 window or emulator.

An action file is a JSON list: {"at": seconds, "op": "tap", "args": [x,y]}.
Supported actions: tap, drag (x,y,x,y,duration), move, scroll (x,y,steps), key, text, screenshot.
Raw captures and the exact action list are retained beside the captioned clip.
"""
import argparse
import csv
import io
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
from PIL import Image


def caption_clip(raw, caption, output, seconds, pre_roll=0):
    # Android stops emitting frames while idle. Hold its final actual frame to
    # the recorded wall-clock duration so measurements remain readable.
    vf = (f"fps=30,tpad=stop_mode=clone:stop_duration={seconds},trim=duration={seconds},"
          "pad=iw:ih+80:0:80:color=0x121214,drawtext="
          "fontfile=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf:textfile="
          + str(caption.resolve()) + ":fontcolor=0xE9E9EF:fontsize=26:x=24:y=25")
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-ss', str(pre_roll), '-i', str(raw),
                    '-vf', vf, '-an', '-c:v', 'libx264', '-preset', 'fast', '-crf', '20',
                    '-pix_fmt', 'yuv420p', '-movflags', '+faststart', '-y', str(output)], check=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('platform', choices=['android', 'desktop'])
    p.add_argument('name')
    p.add_argument('--caption', required=True)
    p.add_argument('--actions', type=Path, required=True)
    p.add_argument('--seconds', type=float, default=18)
    p.add_argument('--out', type=Path, default=Path('showcase/thalassa/highlights'))
    p.add_argument('--serial', default='emulator-5580')
    p.add_argument('--android-size', help='Optional encoder size, e.g. 1440x3120; default uses the device resolution')
    p.add_argument('--android-recorder', choices=['emulator', 'screenrecord'], default='emulator',
                   help='Emulator display recording avoids the guest AVC encoder resolution limit')
    p.add_argument('--require-mesh-triangles', type=int, default=0,
                   help='Android: wait for this many settled triangles before starting capture; requires Layout bounds telemetry')
    p.add_argument('--display', default=':99')
    p.add_argument('--window', default='2097156')
    a = p.parse_args()
    if not re.fullmatch(r'[a-z0-9_-]+', a.name): p.error('Use a simple clip name')
    if not re.fullmatch(r'emulator-\d+', a.serial): p.error('Only isolated emulators are supported')
    if a.android_size and not re.fullmatch(r'\d{2,5}x\d{2,5}', a.android_size):
        p.error('Android size must be WIDTHxHEIGHT')
    if a.display != ':99': p.error('Use the isolated review display :99')
    if not 1 <= a.seconds <= 170: p.error('Clip duration must be 1–170 seconds')
    folder = a.out / a.platform
    rawdir = folder / 'raw'; rawdir.mkdir(parents=True, exist_ok=True)
    frames = folder / 'frames'; frames.mkdir(exist_ok=True)
    emulator_capture = a.platform == 'android' and a.android_recorder == 'emulator'
    raw = rawdir / (a.name + ('.webm' if emulator_capture else '.mp4'))
    actions = json.loads(a.actions.read_text())
    if any(action['at'] < 0 or action['at'] >= a.seconds for action in actions):
        p.error('Every action must start within the clip duration')
    if actions != sorted(actions, key=lambda action: action['at']):
        p.error('Actions must be ordered by their start time')
    adb = [str(Path.home() / 'Android/Sdk/platform-tools/adb'), '-s', a.serial]
    expected_size = None
    if a.platform == 'android':
        if a.android_size:
            expected_size = tuple(map(int, a.android_size.split('x')))
        else:
            with Image.open(io.BytesIO(subprocess.check_output(adb + ['exec-out', 'screencap', '-p']))) as frame:
                expected_size = frame.size
    if a.require_mesh_triangles:
        if a.platform != 'android': p.error('Mesh telemetry is currently available only on Android')
        deadline = time.monotonic() + 60
        while True:
            try:
                telemetry = json.loads(subprocess.check_output(adb + ['exec-out', 'cat',
                    '/data/user/0/com.kingsofalchemy.ringdesigner/files/ringdesigner/layout-debug.json'], timeout=10))
                pid = int(subprocess.check_output(adb + ['shell', 'pidof', 'com.kingsofalchemy.ringdesigner'], timeout=10))
                mesh = telemetry['mesh']
                if telemetry['process_id'] == pid and not mesh['building'] and (mesh['triangles'] or 0) >= a.require_mesh_triangles:
                    break
            except (subprocess.SubprocessError, ValueError, KeyError):
                pass
            if time.monotonic() >= deadline:
                raise RuntimeError('Capture stopped: selected mesh has not settled. Enable Layout bounds, choose Showcase and retry.')
            time.sleep(0.25)
    env = dict(os.environ, DISPLAY=a.display)
    def run(args, **kwargs): return subprocess.run(args, check=True, env=env, **kwargs)
    def screenshot(dest):
        if a.platform == 'android':
            dest.write_bytes(subprocess.check_output(adb+['exec-out','screencap','-p']))
        else:
            run(['ffmpeg','-hide_banner','-loglevel','error','-f','x11grab','-window_id',a.window,
                 '-framerate','1','-i',a.display,'-frames:v','1','-y',str(dest)])
    def control(label, dest):
        screenshot(dest)
        factor=2 if a.platform=='desktop' else 1
        ocr=dest.with_name(dest.stem+'-ocr.png')
        with Image.open(dest) as capture_image:
            box=(0,70,346,600) if a.platform=='desktop' else (0,1800,capture_image.width,2850)
            region=capture_image.crop(box)
            region.resize((region.width*factor,region.height*factor)).save(ocr)
        output=subprocess.check_output(['tesseract',str(ocr),'stdout','--tessdata-dir','/tmp','--psm','6','-c','tessedit_create_tsv=1'],stderr=subprocess.DEVNULL).decode()
        dest.with_suffix('.tsv').write_text(output)
        groups={}
        for word in csv.DictReader(io.StringIO(output),delimiter='\t'):
            if word['level']=='5' and re.search(r'[a-zA-Z0-9]', word['text']):
                key=tuple(word[k] for k in ('block_num','par_num','line_num'))
                groups.setdefault(key,[]).append(word)
        normalize=lambda s: re.sub(r'[^a-z0-9]+',' ',s.lower()).strip()
        wanted=normalize(label)
        matches=[]
        for words in groups.values():
            for begin in range(len(words)):
                for end in range(begin+1,len(words)+1):
                    part=words[begin:end]
                    if normalize(' '.join(w['text'] for w in part))==wanted:
                        left=min(int(w['left']) for w in part);top=min(int(w['top']) for w in part)
                        right=max(int(w['left'])+int(w['width']) for w in part)
                        bottom=max(int(w['top'])+int(w['height']) for w in part)
                        matches.append((box[0]+(left+right)//(2*factor),box[1]+(top+bottom)//(2*factor)))
        if len(matches)!=1:
            # On the small desktop font the button border can confuse "Apply".
            # Its unique "operation" word is still inside the same button.
            if a.platform=='desktop' and label=='Apply operation':
                matches=[]
                for words in groups.values():
                    for word in words:
                        if normalize(word['text'])=='operation':
                            matches.append((box[0]+(int(word['left'])+int(word['width'])//2)//factor,
                                            box[1]+(int(word['top'])+int(word['height'])//2)//factor))
            if len(matches)!=1:
                raise RuntimeError(f'Expected one visible {label!r} control, found {len(matches)} in {dest}')
        return matches[0]
    pre_roll = 1.5 if emulator_capture else 0
    if a.platform == 'desktop':
        title = subprocess.check_output(['xdotool','getwindowname',a.window], env=env).decode()
        if 'RingDesigner' not in title: raise RuntimeError('Capture target is not RingDesigner')
        run(['xdotool','windowfocus',a.window])
        capture = ['ffmpeg','-hide_banner','-loglevel','error','-f','x11grab','-window_id',a.window,
                   '-framerate','30','-i',a.display,'-c:v','libx264','-preset','ultrafast','-crf','18',
                   '-pix_fmt','yuv420p','-y',str(raw)]
    elif emulator_capture:
        reply = subprocess.check_output(adb + ['emu', 'screenrecord', 'start',
            '--size', 'x'.join(map(str, expected_size)), '--bit-rate', '20000000',
            '--fps', '30', '--time-limit', str(int(a.seconds + pre_roll) + 3), str(raw.resolve())])
        if b'KO:' in reply or b'OK' not in reply:
            raise RuntimeError('Emulator recording did not start: ' + reply.decode(errors='replace'))
        capture = None
    else:
        remote = '/sdcard/ringdesigner-' + a.name + '.mp4'
        # Always specify the size: an implicit size lets screenrecord silently
        # fall back to 720p on this emulator's AVC encoder.
        capture = adb + ['shell','screenrecord', '--size', 'x'.join(map(str, expected_size)), '--bit-rate','20000000',
                         '--time-limit',str(int(a.seconds)+3),remote]
    proc = subprocess.Popen(capture, env=env, stdin=subprocess.PIPE) if capture else None
    # The emulator's initial VP9 frames can have visible block artifacts.
    # Preserve this warm-up in the raw capture, before any recorded UI actions.
    if pre_roll:
        time.sleep(pre_roll)
    start = time.monotonic()
    performed = []
    try:
        for action in actions:
            if proc is not None and proc.poll() is not None:
                raise RuntimeError('Recorder exited before the action sequence completed')
            delay = action['at'] - (time.monotonic()-start)
            if delay > 0: time.sleep(delay)
            op, args = action['op'], action.get('args', [])
            performed.append(dict(action, actual_at=round(time.monotonic()-start,3)))
            if op in ('tap_label','assert_label'):
                dest=frames/(a.name+'-control-'+str(len(performed))+'.png')
                try:
                    point=control(args[0],dest)
                except RuntimeError:
                    # Only use a fallback explicitly supplied after reviewing
                    # this control on the target device; assertions still fail.
                    if op != 'tap_label' or len(args) != 3:
                        raise
                    point=tuple(map(int,args[1:]))
                    with Image.open(dest) as shot:
                        if not (0 <= point[0] < shot.width and 0 <= point[1] < shot.height):
                            raise RuntimeError('Verified fallback is outside the capture')
                    performed[-1]['coordinate_fallback']=True
                performed[-1]['detected_point']=point
                if op=='tap_label':
                    if a.platform=='android': run(adb+['shell','input','tap',*map(str,point)])
                    else: run(['xdotool','mousemove',*map(str,point),'click','1'])
            elif op == 'screenshot':
                dest=frames/(a.name+'-'+str(args[0])+'.png')
                screenshot(dest)
            elif a.platform == 'android':
                if op=='tap': command=['tap',*map(str,args)]
                elif op=='drag': command=['swipe',*map(str,args[:4]),str(int(args[4]*1000))]
                elif op=='key': command=['keyevent',*map(str,args)]
                else: raise ValueError('Android text must use real on-screen key taps')
                run(adb+['shell','input',*command],stdout=subprocess.DEVNULL)
            else:
                if op=='tap': run(['xdotool','mousemove',*map(str,args),'click','1'])
                elif op=='move': run(['xdotool','mousemove',*map(str,args)])
                elif op=='scroll':
                    x,y,steps=args
                    button='4' if steps > 0 else '5'
                    run(['xdotool','mousemove',str(x),str(y),'click','--repeat',str(abs(int(steps))),'--delay','80',button])
                elif op=='key': run(['xdotool','key','--clearmodifiers',*map(str,args)])
                elif op=='text': run(['xdotool','type','--clearmodifiers','--delay','70',str(args[0])])
                elif op=='drag':
                    x,y,xx,yy,duration=args
                    run(['xdotool','mousemove',str(x),str(y),'mousedown','1'])
                    try:
                        steps=max(2,int(duration*35))
                        for i in range(1,steps+1):
                            t=i/steps
                            run(['xdotool','mousemove',str(round(x+(xx-x)*t)),str(round(y+(yy-y)*t))])
                            time.sleep(duration/steps)
                    finally: run(['xdotool','mouseup','1'])
                else: raise ValueError(op)
        delay=a.seconds-(time.monotonic()-start)
        if delay>0: time.sleep(delay)
    finally:
        if a.platform=='desktop':
            if proc.poll() is None: proc.communicate(b'q',timeout=15)
        elif emulator_capture:
            reply = subprocess.check_output(adb + ['emu', 'screenrecord', 'stop'])
            if b'KO:' in reply:
                raise RuntimeError('Emulator recording stopped unexpectedly: ' + reply.decode(errors='replace'))
        else:
            run(adb+['shell','pkill','-2','screenrecord'])
            proc.wait(timeout=15)
            run(adb+['pull',remote,str(raw)],stdout=subprocess.DEVNULL)
            run(adb+['shell','rm',remote])
    stream = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-select_streams', 'v:0',
        '-show_entries', 'stream=codec_name,width,height,avg_frame_rate', '-of', 'json', str(raw)]))['streams'][0]
    if expected_size and (stream['width'], stream['height']) != expected_size:
        raise RuntimeError(f'Capture resolution mismatch: expected {expected_size}, got {stream["width"]}x{stream["height"]}')
    caption=folder/(a.name+'.txt');caption.write_text(a.caption)
    capture_name = 'Android emulator display (VP9)' if emulator_capture else ('Android screenrecord' if a.platform=='android' else 'X11 app window')
    metadata={'platform':a.platform,'caption':a.caption,'capture':capture_name,
              'window':a.window if a.platform=='desktop' else a.serial,'seconds':a.seconds,'actions':performed,'raw':str(raw),
              'raw_stream':stream,'pre_roll_seconds':pre_roll}
    (folder/(a.name+'.json')).write_text(json.dumps(metadata,indent=2)+'\n')
    # Pad above the original capture; captions never obscure an app control.
    caption_clip(raw, caption, folder/(a.name+'.mp4'), a.seconds, pre_roll)
    print(folder/(a.name+'.mp4'),flush=True)


if __name__ == '__main__': main()
