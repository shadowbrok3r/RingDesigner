#!/usr/bin/env python3
"""Touch/screenshot harness restricted to RingDesigner in an Android emulator."""
import argparse
import json
import hashlib
from pathlib import Path
import re
import subprocess
import time

SDK = Path.home() / 'Android/Sdk'
PACKAGE = 'com.kingsofalchemy.ringdesigner'
ACTIVITY = PACKAGE + '/com.github.egui_mobile.EguiNativeActivity'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--serial', default='emulator-5580')
    p.add_argument('--out', type=Path, default=Path('/tmp/ringdesigner-mobile-review/screenshots'))
    sub = p.add_subparsers(dest='action', required=True)
    sub.add_parser('launch')
    sub.add_parser('state')
    sub.add_parser('log')
    shot = sub.add_parser('screenshot'); shot.add_argument('name')
    held = sub.add_parser('holdshot'); held.add_argument('x',type=int); held.add_argument('y',type=int); held.add_argument('name')
    snapshot = sub.add_parser('project'); snapshot.add_argument('name')
    seed = sub.add_parser('seed'); seed.add_argument('source',type=Path)
    tap = sub.add_parser('tap'); tap.add_argument('x',type=int); tap.add_argument('y',type=int)
    swipe = sub.add_parser('swipe')
    for name in ('x1','y1','x2','y2'): swipe.add_argument(name,type=int)
    swipe.add_argument('--ms',type=int,default=650)
    install = sub.add_parser('install'); install.add_argument('apk',type=Path)
    rotate = sub.add_parser('rotate'); rotate.add_argument('orientation',choices=['portrait','landscape'])
    scale = sub.add_parser('density'); scale.add_argument('dpi',type=int)
    text = sub.add_parser('text'); text.add_argument('value')
    key = sub.add_parser('key'); key.add_argument('key',choices=['back','enter','delete','home'])
    args = p.parse_args()
    if not re.fullmatch(r'emulator-\d+',args.serial): p.error('Only emulator serials are accepted')
    adb = [str(SDK/'platform-tools/adb'),'-s',args.serial]
    def run(*command, binary=False):
        result = subprocess.run(adb+list(command),check=True,capture_output=True,timeout=45)
        return result.stdout if binary else result.stdout.decode(errors='replace')
    if args.action == 'launch':
        run('shell','am','force-stop',PACKAGE)
        print(run('shell','am','start','-W','-n',ACTIVITY))
        time.sleep(2)
    elif args.action == 'state':
        for command in [('shell','getprop','sys.boot_completed'),('shell','wm','size'),('shell','wm','density'),('shell','pidof',PACKAGE)]:
            print(run(*command).strip())
    elif args.action == 'screenshot':
        if not re.fullmatch(r'[a-zA-Z0-9_.-]+',args.name): p.error('Use a simple screenshot name')
        args.out.mkdir(parents=True,exist_ok=True)
        path=args.out/(args.name if args.name.endswith('.png') else args.name+'.png')
        path.write_bytes(run('exec-out','screencap','-p',binary=True))
        print(path)
    elif args.action == 'holdshot':
        if not re.fullmatch(r'[a-zA-Z0-9_.-]+',args.name): p.error('Use a simple screenshot name')
        proc=subprocess.Popen(adb+['shell','input','swipe',str(args.x),str(args.y),str(args.x),str(args.y),'1800'],stdout=subprocess.DEVNULL)
        try:
            time.sleep(0.45)
            args.out.mkdir(parents=True,exist_ok=True)
            path=args.out/(args.name+'.png');path.write_bytes(run('exec-out','screencap','-p',binary=True));print(path)
        finally: proc.wait(timeout=10)
    elif args.action in ('project','seed'):
        run('root');run('wait-for-device')
        remote='/data/user/0/'+PACKAGE+'/files/ringdesigner/current.ring.json'
        if args.action=='project':
            if not re.fullmatch(r'[a-zA-Z0-9_.-]+',args.name): p.error('Use a simple snapshot name')
            data=run('exec-out','cat',remote,binary=True)
            project=json.loads(data)
            folder=args.out.parent/'projects';folder.mkdir(parents=True,exist_ok=True)
            path=folder/(args.name+'.ring.json');path.write_bytes(data)
            profile=project.get('profile',{})
            print(json.dumps({'path':str(path),'sha256':hashlib.sha256(data).hexdigest(),'name':project.get('name'),'size':project.get('size'),'profile':{k:profile.get(k) for k in ('style','width_mm','thickness_mm')},'layers':len(project.get('layers',{}).get('layers',[]))}))
        else:
            project=json.loads(args.source.read_text())
            if not all(k in project for k in ('profile','shank','layers','size')): p.error('Expected a ring project JSON file')
            run('shell','am','force-stop',PACKAGE)
            print(run('push',str(args.source.resolve()),remote))
            uid=run('shell','stat','-c','%u','/data/user/0/'+PACKAGE).strip()
            if not uid.isdigit(): raise RuntimeError('Cannot determine emulator app UID')
            # adb push can create these directories as system/root before the
            # app's first launch. The app must be able to save prefs and edits.
            run('shell','chown',uid+':'+uid,'/data/user/0/'+PACKAGE+'/files',str(Path(remote).parent))
            run('shell','chown',uid+':'+uid,remote)
            print(run('shell','am','start','-W','-n',ACTIVITY))
            time.sleep(2)
    elif args.action == 'tap':
        print(run('shell','input','tap',str(args.x),str(args.y)))
        time.sleep(0.3)
    elif args.action == 'swipe': print(run('shell','input','swipe',str(args.x1),str(args.y1),str(args.x2),str(args.y2),str(args.ms)))
    elif args.action == 'install': print(run('install','-r',str(args.apk.resolve())))
    elif args.action == 'rotate':
        run('shell','settings','put','system','accelerometer_rotation','0')
        print(run('shell','settings','put','system','user_rotation','1' if args.orientation=='landscape' else '0'))
    elif args.action == 'density':
        if not 320 <= args.dpi <= 720: p.error('Density must be between 320 and 720 dpi')
        print(run('shell','wm','density',str(args.dpi)))
    elif args.action == 'text':
        if not re.fullmatch(r'[a-zA-Z0-9 ._-]{1,100}',args.value): p.error('Text accepts letters, numbers, spaces and ._- only')
        print(run('shell','input','text',args.value.replace(' ','%s')))
    elif args.action == 'key':
        code={'back':'4','enter':'66','delete':'67','home':'3'}[args.key]
        print(run('shell','input','keyevent',code))
    elif args.action == 'log':
        data=run('logcat','-d','-t','2000')
        path=args.out.parent/'app-logcat.txt';path.parent.mkdir(parents=True,exist_ok=True);path.write_text(data)
        for line in data.splitlines():
            if re.search(r'panicked|RustStdoutStderr|ringdesigner.*[Ee]rror|mobile-layout',line): print(line)
        print('Full emulator log:',path)


if __name__ == '__main__': main()
