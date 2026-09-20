"""Illustrate native thumbnail outputs with a project-owned source (no Explorer UI mockup)."""
from pathlib import Path
import argparse, hashlib, json, struct, subprocess
from PIL import Image, ImageDraw, ImageFont


def run(args):
    root=Path(__file__).resolve().parents[2];out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    im=Image.open(root/'crates/iv-viewer/assets/icon.png').convert('RGBA').resize((256,256),Image.Resampling.LANCZOS)
    im.save(out/'Sample.png');im.save(out/'Sample.webp',lossless=True,exact=True);im.save(out/'Sample.avif',quality=100,speed=8)
    header=b'8BPS'+struct.pack('>H6sHIIHH',1,b'\0'*6,4,256,256,8,3)+b'\0'*12+struct.pack('>H',0)
    (out/'Sample.psd').write_bytes(header+b''.join(c.tobytes() for c in im.split()))
    dds=[124,0x100F,256,256,1024,0,0]+[0]*11+[32,0x41,0,32,255,65280,16711680,4278190080]+[0x1000,0,0,0,0]
    (out/'Sample.dds').write_bytes(b'DDS '+struct.pack('<31I',*dds)+im.tobytes())
    formats=['png','webp','avif','psd','dds'];native=out/'native'
    subprocess.run([str(args.probe),str(args.dll),str(native)]+[str(out/('Sample.'+f)) for f in formats],check=True)
    canvas=Image.new('RGB',(880,196),(242,245,250));font=ImageFont.truetype('C:/Windows/Fonts/msyh.ttc',18)
    records=[]
    for i,f in enumerate(formats):
        p=native/f'Sample.{f}-128.json';m=json.loads(p.read_text('utf-8'))
        decoded=Image.frombytes('RGBA',(m['width'],m['height']),p.with_suffix('.bgra').read_bytes(),'raw','BGRA',m['stride'],1)
        assert decoded.size==(128,128) and decoded.getchannel('A').getextrema()[0]<255
        tile=Image.new('RGBA',(128,128));draw=ImageDraw.Draw(tile)
        for y in range(0,128,8):
            for x in range(0,128,8):draw.rectangle((x,y,x+7,y+7),fill=((224,228,234,255) if (x//8+y//8)%2 else (247,248,250,255)))
        tile.alpha_composite(decoded);canvas.paste(tile.convert('RGB'),(24+i*176,16));ImageDraw.Draw(canvas).text((44+i*176,157),f.upper(),font=font,fill=(40,52,70))
        records.append({'format':f,'input_sha256':hashlib.sha256((out/('Sample.'+f)).read_bytes()).hexdigest(),'native_output_sha256':hashlib.sha256(p.with_suffix('.bgra').read_bytes()).hexdigest(),'size':decoded.size})
    dest=root/'docs/screenshots/features/v0.5.0/format-thumbnails.jpg';canvas.save(dest,quality=92,subsampling=0,optimize=True)
    manifest=dest.with_name('manifest.json');data=json.loads(manifest.read_text('utf-8'));data['illustrations'][dest.name]={'sources':records,'dll_sha256':hashlib.sha256(args.dll.read_bytes()).hexdigest(),'method':'Actual COM thumbnail provider outputs, composited only on checkerboard with format captions','bytes':dest.stat().st_size,'size':canvas.size};manifest.write_text(json.dumps(data,ensure_ascii=False,indent=2),'utf-8')
    print('THUMBNAIL_GALLERY_VERIFIED',len(records),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for n in ['dll','probe','output']:p.add_argument('--'+n,type=Path,required=True)
    run(p.parse_args())
