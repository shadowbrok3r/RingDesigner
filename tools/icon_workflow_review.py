#!/usr/bin/env python3
"""Replay the ARTEMIS-verified panel/hold workflow on an authorized emulator.

Requires an installed RingDesigner 0.17+, adb root, and portrait orientation.
Uses app-reported widget bounds; no screen-size-specific click coordinates.
Restores preferences and asserts that these UI operations do not edit the ring.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--serial',required=True)
    p.add_argument('--out',type=Path,default=Path('target/icon-review/replay'))
    a=p.parse_args()
    if not re.fullmatch(r'emulator-\d+',a.serial): p.error('Use an emulator, not a personal phone')
    a.out.mkdir(parents=True,exist_ok=True)
    adb=['adb','-s',a.serial];package='com.kingsofalchemy.ringdesigner'
    folder=f'/data/user/0/{package}/files/ringdesigner'
    def run(*cmd): return subprocess.check_output(adb+list(cmd),timeout=30)
    def stop(): run('shell','am','force-stop',package)
    def start(): run('shell','am','start','-W','-n',package+'/com.github.egui_mobile.EguiNativeActivity')
    run('root');run('wait-for-device');stop()
    prefs=run('exec-out','cat',folder+'/prefs.json')
    original=run('exec-out','cat',folder+'/current.ring.json')
    uid=run('shell','stat','-c','%u',f'/data/user/0/{package}').decode().strip()
    def write_prefs(data):
        local=a.out/'preferences.tmp';local.write_bytes(data)
        run('push',str(local),folder+'/prefs.json');run('shell','chown',uid+':'+uid,folder+'/prefs.json');local.unlink()
    def layout(): return json.loads(run('exec-out','cat',folder+'/layout-debug.json'))
    def wait(test,timeout=12):
        end=time.monotonic()+timeout;last=None
        while time.monotonic()<end:
            try:
                last=layout()
                if test(last):return last
            except (ValueError,subprocess.CalledProcessError):pass
            time.sleep(.12)
        (a.out/'timeout-layout.json').write_text(json.dumps(last,indent=2))
        raise AssertionError(f'UI condition timed out; mode={last and last.get("mode")}, palette={last and last.get("palette")}')
    def rect(label,d=None):
        d=d or layout()
        e=next(e for e in d['controls'] if e['label']==label)
        r,c=e['rect'],e['clip'];return [max(r[0],c[0]),max(r[1],c[1]),min(r[2],c[2]),min(r[3],c[3])]
    def point(label,d=None):
        d=d or layout();r=rect(label,d);scale=d['pointer']['pixels_per_point']
        assert r[2]>r[0] and r[3]>r[1],label+' is clipped'
        return [round((r[0]+r[2])*scale/2),round((r[1]+r[3])*scale/2)]
    def tap(label):
        run('shell','input','tap',*map(str,point(label)))
        # Newly opened egui Areas have a sizing pass before they accept input.
        time.sleep(.4)
    def drag(label,end):
        run('shell','input','swipe',*map(str,point(label)),*map(str,end),'1000')
    def shot(name):
        (a.out/(name+'.png')).write_bytes(run('exec-out','screencap','-p'))
        (a.out/(name+'.json')).write_text(json.dumps(layout(),indent=2))
    results=[]
    try:
        config=json.loads(prefs);config['editor_debug_layout']=True;config['editor_inspector']=True
        config['workspace']['inspector_fraction']=[.4,.36]
        config['workspace']['rail_position']=None;config['workspace']['rail_collapsed']=False
        write_prefs(json.dumps(config).encode());start()
        pid=int(run('shell','pidof',package))
        wait(lambda d:d.get('process_id')==pid and not d['mesh']['building'],90)
        time.sleep(.4)
        for control in ('inspector/expand','header/Panel'):
            d=layout();s=d['pointer']['pixels_per_point'];safe=d['safe_rect']
            drag('inspector/resize',[round(safe[2]*s/2),round((safe[3]-32)*s)])
            wait(lambda d:d['workspace']['inspector_fraction'][0]<.16 and rect('inspector',d)[3]-rect('inspector',d)[1]<100)
            shot('panel-minimum-'+control.split('/')[-1]);tap(control)
            wait(lambda d:d['workspace']['inspector_fraction'][0]>=.3 and rect('inspector',d)[3]-rect('inspector',d)[1]>160)
            results.append(control+' restored the minimized panel')
        tap('floating/More');wait(lambda d:d['palette']=='Some(More)')
        d=layout();s=d['pointer']['pixels_per_point'];safe=d['safe_rect']
        drag('viewport-palette/grip',[round((safe[2]-90)*s),round((safe[3]-65)*s)])
        wait(lambda d:rect('viewport-palette',d)[1]>rect('viewport',d)[3])
        shot('toolbox-over-inspector');results.append('Toolbox moved wholly below the 3D viewport')
        tap('viewport-palette/close');wait(lambda d:d['palette']=='None')
        xy=point('header/Guide')
        held=subprocess.Popen(adb+['shell','input','swipe',*map(str,xy),*map(str,xy),'2200'])
        time.sleep(1.1);shot('finger-hold-help');held.wait(timeout=10)
        time.sleep(.2)
        assert abs(layout()['workspace']['inspector_fraction'][0]-.42)>.002,'Hold unexpectedly opened Guide'
        results.append('Finger hold released without opening Guide')
        tap('header/Guide');wait(lambda d:abs(d['workspace']['inspector_fraction'][0]-.42)<.002)
        shot('workflow-guide');results.append('Guide opens with a normal tap')
        now=run('exec-out','cat',folder+'/current.ring.json')
        assert hashlib.sha256(now).digest()==hashlib.sha256(original).digest(),'UI navigation modified the design'
        results.append('Design bytes unchanged')
        (a.out/'results.json').write_text(json.dumps({'passed':results},indent=2))
        print(json.dumps({'passed':results},indent=2))
    finally:
        stop();write_prefs(prefs);start()

if __name__=='__main__':main()
