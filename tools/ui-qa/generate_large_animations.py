"""Generate controlled GIF/APNG/WebP fixtures whose decoded frames exceed 256 MiB.
No user images are read or modified. Each format is placed in a separate folder to
avoid speculative cross-decoding during GUI memory measurements.
"""
from pathlib import Path
import argparse, json, struct
from PIL import Image, ImageDraw

def generate(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=False)
    frames = []
    for i in range(72):
        color = ((i*3+12)%256, (i*7+20)%256, (i*11+30)%256, 255)
        im = Image.new('RGBA', (1024,1024), color)
        d = ImageDraw.Draw(im)
        d.rectangle((32,32,180,180), fill=(240,240,240,255))
        d.text((52,80), f'FRAME {i+1}/72', fill=(0,0,0,255), font_size=20)
        # A transparent region exercises original alpha without changing the center.
        d.rectangle((900,0,1023,123), fill=(0,0,0,0))
        frames.append(im)
    durations = [40 + (i%3)*10 for i in range(72)]
    for ext,options in [('gif',dict(disposal=2,optimize=False)),('png',dict(disposal=0,blend=0)),('webp',dict(lossless=True,method=0))]:
        folder=out/ext;folder.mkdir();p=folder/('large-animation.'+ext)
        frames[0].save(p,save_all=True,append_images=frames[1:],duration=durations,loop=0,**options)
        with Image.open(p) as check:
            assert check.n_frames == 72
            samples=[]
            for i in range(check.n_frames):
                check.seek(i);rgba=check.convert('RGBA')
                samples.append({'center':list(rgba.getpixel((512,512))), 'alpha':rgba.getpixel((950,60))[3], 'delay':durations[i]})
        (folder/'expected.json').write_text(json.dumps(samples),'utf-8')
        (folder/'expected.bin').write_bytes(b''.join(bytes(x['center'])+bytes([x['alpha']])+struct.pack('<I',x['delay']) for x in samples))
        print(json.dumps({'path':str(p),'frames':72,'dimensions':[1024,1024],
            'raw_frame_bytes':72*1024*1024*4,'file_bytes':p.stat().st_size}),flush=True)
    for im in frames:im.close()

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path)
    generate(p.parse_args().output)
