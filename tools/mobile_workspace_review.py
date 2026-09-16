#!/usr/bin/env python3
"""Drive visible, named editor controls in the isolated Android review emulator.

Enable View > Layout bounds (debug) first. Coordinates come from the actual UI,
including its clip rectangles, rather than a fixed phone screenshot.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess
import time

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial',default='emulator-5580')
    parser.add_argument('--out',type=Path,default=Path('/tmp/ringdesigner-014-review'))
    sub=parser.add_subparsers(dest='action',required=True)
    sub.add_parser('state')
    snap=sub.add_parser('snapshot');snap.add_argument('name')
    tap=sub.add_parser('tap');tap.add_argument('label');tap.add_argument('--index',type=int,default=0)
    drag=sub.add_parser('drag');drag.add_argument('label');drag.add_argument('dx',type=float);drag.add_argument('dy',type=float);drag.add_argument('--index',type=int,default=0)
    args=parser.parse_args()
    if not re.fullmatch(r'emulator-\d+',args.serial):parser.error('Only emulator devices are accepted')
    adb=[str(Path.home()/'Android/Sdk/platform-tools/adb'),'-s',args.serial]
    root='/data/user/0/com.kingsofalchemy.ringdesigner/files/ringdesigner/'
    def run(*cmd):return subprocess.check_output(adb+list(cmd),timeout=30)
    def layout():
        for _ in range(10):
            try:
                data=json.loads(run('exec-out','cat',root+'layout-debug.json'))
                pid=int(run('shell','pidof','com.kingsofalchemy.ringdesigner').strip())
                if data.get('process_id') != pid:
                    raise RuntimeError('Enable Layout bounds in the current app session; stored report is stale')
                return data
            except json.JSONDecodeError:time.sleep(0.1)
        raise RuntimeError('Enable View > Layout bounds (debug) in the app')
    data=layout()
    if args.action in ('tap','drag'):
        matches=[c for c in data['controls'] if c['label']==args.label]
        if not 0<=args.index<len(matches):raise RuntimeError('Visible control not found: '+args.label)
        c=matches[args.index];r=c['rect'];clip=c['clip']
        left,top,right,bottom=max(r[0],clip[0]),max(r[1],clip[1]),min(r[2],clip[2]),min(r[3],clip[3])
        if right-left<2 or bottom-top<2:raise RuntimeError('Control is clipped; scroll it into view first')
        density=run('shell','wm','density').decode()
        scale=int(re.findall(r'density: (\d+)',density)[-1])/160.0
        x,y=(left+right)*0.5,(top+bottom)*0.5
        point=lambda v:str(round(v*scale))
        if args.action=='tap':run('shell','input','tap',point(x),point(y))
        else:run('shell','input','swipe',point(x),point(y),point(x+args.dx),point(y+args.dy),'650')
        time.sleep(0.6)
        data=layout()
    elif args.action=='snapshot':
        if not re.fullmatch(r'[a-zA-Z0-9_-]+',args.name):parser.error('Use a simple snapshot name')
        args.out.mkdir(parents=True,exist_ok=True)
        (args.out/(args.name+'.layout.json')).write_text(json.dumps(data,indent=2)+'\n')
        (args.out/(args.name+'.png')).write_bytes(run('exec-out','screencap','-p'))
        (args.out/(args.name+'.ring.json')).write_bytes(run('exec-out','cat',root+'current.ring.json'))
        print('Saved',args.out/args.name)
    print(json.dumps({k:data[k] for k in ['mode','palette','workspace','camera','overflow']},indent=2))
    if args.action=='state':
        for c in data['controls']:print(c['label'],c['rect'])

if __name__=='__main__':main()
