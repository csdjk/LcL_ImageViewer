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
            inputs=[(name,root/'crates/iv-core/tests/fixtures/avif'/name) for name in ['alpha.avif','animated-alpha.avif']]
            inputs.append(('Artwork.png',root/'crates/iv-viewer/assets/icon.png'))
            for name, sample in inputs:
                case={'input':name,'launch_alive':False,'screenshots':[],'visual_capture_available':False}
                env=dict(os.environ,LCL_IV_QA_PROFILE='macos-'+uuid.uuid4().hex,RUST_BACKTRACE='1')
                with (out/f'{name}.log').open('wb') as log:
                    proc=subprocess.Popen([str(binary),str(sample)],env=env,stdout=log,stderr=subprocess.STDOUT)
                try:
                    time.sleep(4)
                    assert proc.poll() is None, (name,'app exited',proc.returncode,(out/f'{name}.log').read_text(errors='replace'))
                    case['launch_alive']=True
                    def capture_state(label, key=None):
                        capture=out/f'{name}-{label}.png'
                        argv=[str(helper),str(proc.pid),str(capture)]
                        if key is not None: argv.append(str(key))
                        result=subprocess.run(argv,capture_output=True,text=True,timeout=30)
                        if result.returncode==0 and capture.exists():
                            info=json.loads(result.stdout);case['screenshots'].append(info);case['visual_capture_available']=True
                            return info
                        case['capture_error']=result.stderr[-2000:]
                        return None
                    first=capture_state('initial')
                    if first and first.get('accessibility_trusted'):
                        pixels=lambda info: tuple(tuple(c) for c in info['image_samples_rgb'])
                        if name=='alpha.avif':
                            baseline=pixels(first)
                            for code, label in [(18,'red'),(19,'green'),(20,'blue'),(21,'alpha')]:
                                channel=capture_state(label,code)
                                assert channel and all(max(c)-min(c)<=1 for c in pixels(channel)), (label,'not grayscale')
                            restored=capture_state('rgba-restored',23)
                            assert restored and pixels(restored)==baseline,'RGBA restoration changed image pixels'
                            case['rgba_channel_keyboard_verified']=True
                        elif name=='animated-alpha.avif':
                            samples=[]
                            for index,delay in enumerate([0.013,0.137,0.271,0.409,0.073]):
                                time.sleep(delay);shot=capture_state('playing-'+str(index));assert shot
                                samples.append(pixels(shot))
                            assert len(set(samples))>=2,'Animation did not change rendered image pixels'
                            paused=capture_state('paused',49);assert paused
                            time.sleep(0.43);held=capture_state('paused-held');assert held
                            assert pixels(paused)==pixels(held),'Paused animation did not hold'
                            stepped=capture_state('next-frame',47);assert stepped
                            assert pixels(stepped)!=pixels(paused),'Frame-step did not change image'
                            capture_state('resume',49)
                            case['animation_play_pause_step_verified']=True
                    elif not first:
                        case['interaction_verification']='Unavailable: runner window capture permission'
                    else:
                        case['interaction_verification']='Unavailable: runner Accessibility permission'
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
