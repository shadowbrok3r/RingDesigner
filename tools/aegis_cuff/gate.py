import json, sys, time, os, random, urllib.request, urllib.parse, urllib.error
env = {}
for line in open(os.path.expanduser('~/.config/ringdesigner/comfy.env')):
    line = line.strip()
    if '=' in line and not line.startswith('#'):
        k, v = line.split('=', 1); env[k.strip()] = v.strip().strip('"').strip("'")
BASE = env.get('COMFY_GATE_URL') or 'https://comfy.shadowbroker.app'
KEY = env['COMFY_GATE_KEY']
WF_DIR = '/home/shadowbroker/Documents/Rust/JewelryProjects/RingDesigner/crates/ringdesign-gui/src/comfy/'
PRESET = os.environ.get('GATE_PRESET', 'ring_band_strip')
WF = WF_DIR + PRESET + '.api.json'
def call(method, path, body=None, raw=False):
    req = urllib.request.Request(BASE + path, method=method, data=None if body is None else json.dumps(body).encode(),
                                 headers={'x-api-key': KEY, 'content-type': 'application/json', 'user-agent': 'ringdesigner'})
    try:
        with urllib.request.urlopen(req, timeout=180) as r:
            data = r.read()
    except urllib.error.HTTPError as e:
        raise SystemExit(f'{method} {path} -> {e.code}: {e.read()[:300]!r}')
    return data if raw else json.loads(data)
def submit(prompt, seed):
    nodes = json.load(open(WF))['prompt']
    pos = next(n['inputs']['positive'][0] for n in nodes.values() if n['class_type'] == 'KSampler')
    text = prompt if prompt.lower().startswith('bas-relief') else 'BAS-RELIEF page, ' + prompt
    for nid, n in nodes.items():
        c, i = n['class_type'], n['inputs']
        if c == 'CLIPTextEncode' and nid == pos: i['text'] = text
        elif c == 'KSampler': i.update(seed=seed, steps=30, cfg=5.0)
        elif c == 'LoraLoader' and 'BAS-RELIEF' in i.get('lora_name', ''): i.update(strength_model=0.8, strength_clip=0.8)
        elif c in ('SaveImage', 'SaveImageAdvanced') and isinstance(i.get('filename_prefix'), str):
            i['filename_prefix'] = 'texture/' + i['filename_prefix'].rsplit('/', 1)[-1]
    q = call('POST', '/api/prompt', {'prompt': nodes, 'client_id': 'ringdesigner'})
    if 'error' in q: raise SystemExit(f"rejected: {q['error']}")
    return q['prompt_id']
def wait(pid, timeout=900):
    t0 = time.time()
    while time.time() - t0 < timeout:
        h = call('GET', f'/api/history/{pid}')
        e = h.get(pid)
        if e:
            st = e.get('status', {})
            if st.get('status_str') == 'error': raise SystemExit(f'render failed: {st.get("messages")}')
            if st.get('completed'):
                outs = []
                for node in e.get('outputs', {}).values():
                    for im in node.get('images', []):
                        if im.get('type') != 'temp': outs.append(im)
                return outs
        time.sleep(2)
    raise SystemExit('timed out')
def fetch(outs, stem, path):
    o = next(o for o in outs if stem in o['filename'])
    q = urllib.parse.urlencode({'filename': o['filename'], 'subfolder': o['subfolder'], 'type': 'output'})
    open(path, 'wb').write(call('GET', '/api/view?' + q, raw=True))
if __name__ == '__main__':
    prompt = open(sys.argv[1]).read().strip(); tag = sys.argv[2]; seeds = [int(x) for x in sys.argv[3:]]
    ids = [(s, submit(prompt, s)) for s in seeds]
    print('queued', [s for s, _ in ids], flush=True)
    for s, pid in ids:
        outs = wait(pid)
        fetch(outs, PRESET + '_height16', f'{tag}_{s}_height16.png')
        try: fetch(outs, PRESET + '_color', f'{tag}_{s}_color.png')
        except StopIteration: pass
        print('got', s, flush=True)
