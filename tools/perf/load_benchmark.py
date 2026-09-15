"""Warm-filesystem, fresh-process Windows load benchmark using opt-in IVPERF events.
Only creates new fixtures/QA profiles and closes processes it launches. No user images,
registry, install paths or global filesystem-cache state are modified.
Image-paint-queued is a CPU submission marker, not monitor/photon latency.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import statistics
import subprocess
import sys
import time
import uuid
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/ui-qa'))
import neumorphic_smoke as qa


def generate(folder: Path) -> None:
    folder.mkdir(parents=True, exist_ok=False)
    rng = random.Random(915)
    # Actual encoded PNG/JPEG inputs, generated once and shared between A and B.
    for size, label in [(1024, 'small'), (4096, 'large')]:
        group = folder / label
        group.mkdir()
        image = Image.frombytes('RGBA', (size, size), rng.randbytes(size * size * 4))
        image.save(group / f'{label}.png', compress_level=4)
        if size == 4096:
            jpg = folder / 'jpeg'
            jpg.mkdir()
            image.convert('RGB').save(jpg / 'photo.jpg', quality=90)
    dense = folder / 'dense'; dense.mkdir()
    for i in range(12):
        image = Image.new('RGBA', (1024, 1024), (45+i*9, 120, 190, 255))
        draw = ImageDraw.Draw(image)
        draw.rectangle((150, 200, 850, 850), fill=(180, 80+i*9, 45, 180))
        draw.text((500, 500), f'FRAME {i}', fill=(255, 255, 255, 255))
        image.save(dense / f'{i:02}.png')
    for i in range(2000):
        (dense / f'irrelevant_{i:04}.txt').write_text('', encoding='utf-8')
    manifest = {str(p.relative_to(folder)): {'bytes': p.stat().st_size,
                 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
                for p in folder.rglob('*') if p.is_file() and p.suffix != '.txt'}
    (folder / 'manifest.json').write_text(json.dumps(manifest, indent=2), encoding='utf-8')


def events(log: Path) -> list[dict]:
    result = []
    for line in log.read_text(encoding='utf-8', errors='replace').splitlines():
        parts = line.split('\t')
        if len(parts) == 5 and parts[0] == 'IVPERF':
            result.append({'ms': float(parts[1]), 'event': parts[2],
                           'file': parts[3], 'stage_ms': float(parts[4])})
    return result


def run(args):
    fixtures = args.fixtures.resolve()
    if args.generate:
        generate(fixtures)
    if not args.binary:
        return
    binary = args.binary.resolve()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    runs = []
    for group, filename in [('small','small.png'),('large','large.png'),('jpeg','photo.jpg'),('dense','00.png')]:
        path = fixtures / group / filename
        for sample in range(args.samples):
            profile = 'perf-' + uuid.uuid4().hex
            storage = Path(os.environ['APPDATA']) / ('LcL ImageViewer QA-' + profile) / 'data/app.ron'
            storage.parent.mkdir(parents=True, exist_ok=False)
            storage.write_text(json.dumps({'iv-theme':'dark','iv-backdrop':'off','iv-checkerboard':'on','iv-reduce-motion':'off'}), encoding='utf-8')
            logpath = output / f'{group}-{sample}.log'
            env = dict(os.environ, LCL_IV_QA_PROFILE=profile, LCL_IV_PERF='1')
            with logpath.open('wb') as log:
                proc = subprocess.Popen([str(binary), str(path)], cwd=binary.parent,
                                        env=env, stdout=log, stderr=log)
            try:
                deadline=time.monotonic()+30
                while time.monotonic()<deadline:
                    data=events(logpath)
                    if any(e['event']=='image_paint_queued' and e['file']==filename for e in data):
                        break
                    if proc.poll() is not None:
                        raise RuntimeError(f'Viewer exited {proc.returncode}: {logpath}')
                    time.sleep(.01)
                else: raise TimeoutError(logpath)
                if group=='dense':
                    # Let neighbour prefetch settle, then navigate the same deterministic sequence.
                    time.sleep(.4)
                    handles=qa.windows_for_pid(proc.pid)
                    if len(handles)!=1: raise RuntimeError(handles)
                    hwnd=handles[0]
                    for target in range(1,9):
                        qa.focus(hwnd)
                        qa.u.keybd_event(ord('D'),0,0,0)
                        qa.u.keybd_event(ord('D'),0,2,0)
                        until=time.monotonic()+10
                        while time.monotonic()<until:
                            if any(e['event']=='image_paint_queued' and e['file']==f'{target:02}.png' for e in events(logpath)):
                                break
                            time.sleep(.005)
                        else: raise TimeoutError(f'Navigation target {target}')
                        time.sleep(.06)
                data=events(logpath)
                runs.append({'case':group,'sample':sample,'events':data})
                print(f'{group} {sample}: '+str(next(e['ms'] for e in data if e['event']=='image_paint_queued')), flush=True)
            finally:
                if proc.poll() is None:
                    for hwnd in qa.windows_for_pid(proc.pid):
                        qa.u.PostMessageW(hwnd, 0x0010, 0, 0)
                    try: proc.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        proc.terminate(); proc.wait(timeout=5)
    summary={}
    for case in ('small','large','jpeg','dense'):
        values=[]; waits=[]; decode=[]; scan=[]; upload=[]
        for run in (r for r in runs if r['case']==case):
            data=run['events']; values.append(next(e['ms'] for e in data if e['event']=='image_paint_queued'))
            requested={}
            for e in data:
                if e['event']=='load_request': requested[e['file']]=e['ms']
                if e['event']=='image_paint_queued' and e['file'] in requested:
                    waits.append(e['ms']-requested[e['file']])
                if e['event']=='decode_ready': decode.append(e['stage_ms'])
                if e['event']=='directory_ready': scan.append(e['stage_ms'])
                if e['event']=='image_ready': upload.append(e['stage_ms'])
        def stats(x):
            return {'n':len(x),'median_ms':round(statistics.median(x),3),
                    'p95_ms':round(sorted(x)[min(len(x)-1,int(len(x)*.95))],3)} if x else None
        summary[case]={'startup_to_image_queued':stats(values),'request_to_image_queued':stats(waits),
                       'decode':stats(decode),'scan':stats(scan),'cpu_upload':stats(upload)}
    report={'binary':str(binary),'sha256':qa.sha(binary),'commit':args.commit,
            'sample_count':args.samples,'definition':'Fresh process with warmed filesystem; CPU draw submission, not monitor presentation.',
            'fixtures':json.loads((fixtures/'manifest.json').read_text()),'summary':summary,'runs':runs}
    (output/'report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--fixtures',type=Path,required=True);p.add_argument('--generate',action='store_true')
    p.add_argument('--binary',type=Path);p.add_argument('--output',type=Path);p.add_argument('--commit',default='unknown')
    p.add_argument('--samples',type=int,default=5)
    run(p.parse_args())
