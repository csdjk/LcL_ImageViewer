"""Mount the produced DMG, verify payload and launch the native macOS executable.
Screenshots/keyboard checks run when the hosted runner grants the needed permissions.
Permission limits are reported explicitly, never interpreted as passing visual QA.
"""
from __future__ import annotations
import argparse, hashlib, json, os, plistlib, signal, subprocess, tempfile, time, uuid
from pathlib import Path


def run(args):
    root=Path(__file__).resolve().parents[2];out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    manifest=json.loads((root/'dist/macos/build-manifest.json').read_text('utf-8'))
    helper=out/'window-probe'
    subprocess.run(['swiftc',str(root/'tools/macos/window_probe.swift'),'-o',str(helper)],check=True)
    dmg=root/'dist/macos'/manifest['dmg']['name'];report={'source_commit':manifest['source_commit'],'architecture':manifest['architecture'],'signing':manifest['signing'],'notarized':False,'cases':[]}
    with tempfile.TemporaryDirectory(prefix='lcl-mac-mount-') as temp:
        mount=Path(temp)/'volume';mount.mkdir()
        subprocess.run(['hdiutil','attach','-readonly','-nobrowse','-mountpoint',str(mount),str(dmg)],check=True)
        try:
            app=mount/'LcL ImageViewer.app';binary=app/'Contents/MacOS/imageview'
            assert hashlib.sha256(binary.read_bytes()).hexdigest()==manifest['signed_binary_sha256']
            subprocess.run(['codesign','--verify','--deep','--strict',str(app)],check=True)
            plist=plistlib.loads((app/'Contents/Info.plist').read_bytes())
            assert plist['CFBundleShortVersionString']==manifest['version'] and (app/'Contents/Resources/LcLImageViewer.icns').is_file()
            assert (mount/'Applications').is_symlink()
            for name in ['alpha.avif','animated-alpha.avif']:
                case={'input':name,'launch_alive':False,'screenshots':[],'visual_capture_available':False}
                sample=root/'crates/iv-core/tests/fixtures/avif'/name
                env=dict(os.environ,LCL_IV_QA_PROFILE='macos-'+uuid.uuid4().hex,RUST_BACKTRACE='1')
                with (out/f'{name}.log').open('wb') as log:
                    proc=subprocess.Popen([str(binary),str(sample)],env=env,stdout=log,stderr=subprocess.STDOUT)
                try:
                    time.sleep(4)
                    assert proc.poll() is None, (name,'app exited',proc.returncode,(out/f'{name}.log').read_text(errors='replace'))
                    case['launch_alive']=True
                    for index in range(3):
                        capture=out/f'{name}-{index}.png'
                        result=subprocess.run([str(helper),str(proc.pid),str(capture)],capture_output=True,text=True,timeout=30)
                        if result.returncode==0 and capture.exists():
                            info=json.loads(result.stdout);case['screenshots'].append(info);case['visual_capture_available']=True
                        else:
                            case['capture_error']=result.stderr[-2000:];break
                        time.sleep(0.2)
                    assert proc.poll() is None,'App exited during capture'
                    runtime=(out/f'{name}.log').read_text(errors='replace')
                    assert 'panicked at' not in runtime and 'Validation Error' not in runtime
                    case['runtime_error_free']=True
                finally:
                    if proc.poll() is None:
                        proc.send_signal(signal.SIGTERM)
                        try: proc.wait(timeout=10)
                        except subprocess.TimeoutExpired:proc.kill();proc.wait()
                report['cases'].append(case)
            report['mounted_payload_verified']=True;report['result']='PASS native tests, bundle and startup; visual review separately required'
        finally:
            subprocess.run(['hdiutil','detach',str(mount)],check=True)
    (out/'smoke.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False,indent=2),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--app',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    run(p.parse_args())
