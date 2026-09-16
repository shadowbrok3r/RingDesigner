#!/usr/bin/env python3
"""Assemble reviewed window clips from an explicit edit list, preserving raw takes."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def run(args):
    return subprocess.run([str(x) for x in args], check=True, capture_output=True, text=True)


def probe(path):
    return json.loads(run(['ffprobe', '-v', 'error', '-show_format', '-show_streams',
                           '-of', 'json', path]).stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('edit_list', type=Path)
    parser.add_argument('--platform', choices=['android', 'desktop'])
    parser.add_argument('--name', default='thalassa', help='Output filename stem')
    parser.add_argument('--title', default='Thalassa', help='Design name in video metadata')
    args = parser.parse_args()
    if not re.fullmatch(r'[a-z0-9_-]+', args.name):
        parser.error('Use a simple output name')
    root = args.edit_list.resolve().parent
    edits = json.loads(args.edit_list.read_text())
    for platform, chapters in edits.items():
        if args.platform and platform != args.platform:
            continue
        work = root / platform / 'assembled'
        work.mkdir(exist_ok=True)
        paths, manifest, clock = [], [], 0
        title = args.title.replace('\\', '\\\\').replace('=', '\\=').replace(';', '\\;').replace('#', '\\#').replace('\n', ' ')
        metadata = [';FFMETADATA1', f'title={title} - {platform} application creation reel']
        for chapter in chapters:
            source = root / platform / (chapter['clip'] + '.mp4')
            path = source
            if chapter.get('keep'):
                path = work / source.name
                expressions = []
                for start, end in chapter['keep']:
                    assert 0 <= start < end
                    expressions.append(f'between(t,{float(start)},{float(end)})')
                vf = "select='" + '+'.join(expressions) + "',setpts=N/(30*TB)"
                run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-i', source,
                     '-vf', vf, '-an', '-c:v', 'libx264', '-preset', 'fast', '-crf', '20',
                     '-pix_fmt', 'yuv420p', '-movflags', '+faststart', '-y', path])
            info = probe(path)
            video = next(s for s in info['streams'] if s['codec_type'] == 'video')
            assert video['codec_name'] == 'h264' and video['pix_fmt'] == 'yuv420p'
            assert video['r_frame_rate'] == '30/1'
            duration = round(float(info['format']['duration']) * 1000)
            metadata += ['[CHAPTER]', 'TIMEBASE=1/1000', f'START={clock}',
                         f'END={clock + duration}', f'title={chapter["title"]}']
            manifest.append(dict(chapter, start_ms=clock, duration_ms=duration,
                                 source_sha256=hashlib.file_digest(source.open('rb'), 'sha256').hexdigest()))
            clock += duration
            paths.append(path)
        playlist = work / 'clips.ffconcat'
        playlist.write_text('ffconcat version 1.0\n' + ''.join(
            "file '" + str(p).replace("'", "'\\''") + "'\n" for p in paths))
        meta = work / 'chapters.ffmetadata'
        meta.write_text('\n'.join(metadata) + '\n')
        result = root / f'{args.name}-{platform}.mp4'
        run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-f', 'concat', '-safe', '0',
             '-i', playlist, '-i', meta, '-map', '0:v:0', '-map_metadata', '1',
             '-map_chapters', '1', '-c', 'copy', '-movflags', '+faststart', '-y', result])
        # Decode every frame, in addition to the separate visual review.
        run(['ffmpeg', '-v', 'error', '-xerror', '-i', result, '-map', '0:v:0', '-f', 'null', '-'])
        info = probe(result)
        (root / f'{args.name}-{platform}.json').write_text(json.dumps(dict(
            capture='Real application window/screen; idle intervals may be trimmed',
            chapters=manifest, streams=info['streams'], duration=info['format']['duration'],
            sha256=hashlib.file_digest(result.open('rb'), 'sha256').hexdigest(),
            full_decode_passed=True), indent=2) + '\n')
        print(result, info['format']['duration'] + ' seconds', flush=True)


if __name__ == '__main__':
    main()
