#!/usr/bin/env python3
"""Create and run an isolated S26-size Android UI review device (not Samsung firmware)."""
import argparse
import os
from pathlib import Path
import subprocess

NAME = "ringdesigner-s26-review"

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["init", "start"])
    parser.add_argument("--sdk", type=Path, default=Path.home()/"Android/Sdk")
    parser.add_argument("--root", type=Path, default=Path("/tmp/ringdesigner-mobile-review"))
    parser.add_argument("--port", type=int, default=5580)
    parser.add_argument("--window", action="store_true", help="Show the emulator window")
    args = parser.parse_args()
    if args.port < 5554 or args.port > 5682 or args.port % 2:
        parser.error("Choose an even emulator port from 5554 to 5682")
    root = args.root.resolve()
    avds = root/"avd"
    device = avds/(NAME+".avd")
    config = device/"config.ini"
    system = args.sdk/"system-images/android-36/google_apis/x86_64"
    if not (system/"system.img").exists():
        parser.error("Install system-images;android-36;google_apis;x86_64 with Android SDK Manager first")
    if args.action == "init":
        if config.exists():
            print("Existing isolated device preserved:",config)
            return
        device.mkdir(parents=True, exist_ok=True)
        values = {
            "avd.ini.encoding":"UTF-8", "avd.id":NAME, "avd.name":"RingDesigner S26 display review",
            "abi.type":"x86_64", "hw.cpu.arch":"x86_64", "hw.cpu.ncore":"4",
            "hw.ramSize":"4096", "hw.lcd.width":"1440", "hw.lcd.height":"3120",
            "hw.lcd.density":"560", "hw.keyboard":"no", "hw.gpu.enabled":"yes",
            "hw.gpu.mode":"swiftshader", "hw.mainKeys":"no", "hw.battery":"yes",
            "hw.accelerometer":"yes", "hw.audioInput":"no", "hw.camera.back":"none",
            "hw.camera.front":"none", "image.sysdir.1":str(system.resolve())+"/",
            "tag.id":"google_apis", "tag.display":"Google APIs", "PlayStore.enabled":"no",
            "disk.dataPartition.size":"4G", "showDeviceFrame":"no",
            "fastboot.forceColdBoot":"yes", "fastboot.forceFastBoot":"no",
        }
        config.write_text("".join(f"{k}={v}\n" for k,v in values.items()))
        (avds/(NAME+".ini")).write_text(f"avd.ini.encoding=UTF-8\npath={device}\ntarget=android-36\n")
        print(config)
    else:
        if not config.exists(): parser.error("Run init first")
        if not os.access("/dev/kvm",os.R_OK|os.W_OK):
            parser.error("KVM is unavailable here. Run outside the sandbox with access to /dev/kvm")
        env = os.environ.copy()
        env["ANDROID_AVD_HOME"] = str(avds)
        command = [str(args.sdk/"emulator/emulator"),"-avd",NAME,"-port",str(args.port),
                   "-no-audio","-no-boot-anim","-no-snapshot","-gpu","swiftshader","-accel","on"]
        if not args.window: command.append("-no-window")
        print(f"Starting emulator-{args.port}; Ctrl+C stops this isolated device",flush=True)
        try: subprocess.run(command, env=env, check=True)
        except KeyboardInterrupt: pass

if __name__ == "__main__": main()
