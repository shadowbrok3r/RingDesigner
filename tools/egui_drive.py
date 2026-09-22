#!/usr/bin/env python3
"""Drive the running desktop app through egui-mcp over stdio.

The app must be built with `--features eframe/inspection` and started with
`EGUI_INSPECTION=1` (serves 127.0.0.1:5719). `egui-mcp` comes from
`cargo install egui_mcp`.

usage: egui_drive.py '[["tool", {args}], ...]'     attach is implicit
       LIMIT=20000 egui_drive.py '[["widget_tree", {"role": "Button"}]]'

Tools: status, resize, screenshot{save_path}, widget_tree, get_widget, click,
hover, drag, scroll, type_text, press_key, wait_for. Every call is printed
with its text result; images are reported by size and saved via save_path.
"""
import json
import os
import subprocess
import sys


def main() -> int:
    calls = json.loads(sys.argv[1])
    server = subprocess.Popen(
        [os.path.expanduser("~/.cargo/bin/egui-mcp")],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    serial = 0

    def rpc(method, params=None, notify=False):
        nonlocal serial
        message = {"jsonrpc": "2.0", "method": method}
        if params is not None:
            message["params"] = params
        if not notify:
            serial += 1
            message["id"] = serial
        server.stdin.write(json.dumps(message) + "\n")
        server.stdin.flush()
        if notify:
            return None
        while True:
            line = server.stdout.readline()
            if not line:
                raise SystemExit("egui-mcp closed")
            reply = json.loads(line)
            if reply.get("id") == serial:
                return reply

    def call(tool, args):
        reply = rpc("tools/call", {"name": tool, "arguments": args})
        if "error" in reply:
            return "RPC ERROR " + json.dumps(reply["error"])
        out = []
        for item in reply["result"].get("content", []):
            if item.get("type") == "text":
                out.append(item["text"])
            elif item.get("type") == "image":
                out.append("[image %d b64 bytes]" % len(item.get("data", "")))
        if reply["result"].get("isError"):
            out.insert(0, "TOOL ERROR")
        return "\n".join(out)

    rpc(
        "initialize",
        {
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {"name": "egui_drive", "version": "0"},
        },
    )
    rpc("notifications/initialized", notify=True)
    attached = call("attach", {"timeout_secs": 15})
    if "rror" in attached:
        print(attached)
        server.terminate()
        return 1
    limit = int(os.environ.get("LIMIT", "6000"))
    failed = False
    for tool, args in calls:
        result = call(tool, args)
        failed |= result.startswith(("TOOL ERROR", "RPC ERROR"))
        print(f"== {tool} {json.dumps(args)[:120]}\n{result[:limit]}")
    server.terminate()
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
