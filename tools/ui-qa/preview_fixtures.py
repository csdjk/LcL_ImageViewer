"""Create new deterministic PNG/WebP/PSD fixtures; never touches user's images."""
from pathlib import Path
from PIL import Image, ImageDraw
import struct
import json
import hashlib
import argparse


def make(folder: Path):
    folder.mkdir(parents=True, exist_ok=False)
    im = Image.new('RGBA', (320, 192), (180, 90, 30, 0))
    d = ImageDraw.Draw(im)
    d.rectangle((40, 40, 139, 151), fill=(220, 80, 50, 255))
    d.rectangle((140, 40, 219, 151), fill=(50, 160, 220, 128))
    d.rectangle((220, 40, 279, 151), fill=(80, 200, 100, 64))
    im.save(folder / '01-transparent.png')
    im.save(folder / '02-static.webp', lossless=True, exact=True)
    second=Image.new('RGBA', im.size, (60, 100, 230, 128))
    im.save(folder / '03-animated.webp', lossless=True, exact=True, save_all=True,
            append_images=[second],duration=[80,120],loop=0)
    planes=[channel.tobytes() for channel in im.split()]
    for name, rle, extra in [('04-raw.psd',False,False),('05-rle-extra.psd',True,True)]:
        ps=planes + ([bytes([110])* (320*192)] if extra else [])
        header=b'8BPS'+struct.pack('>H6sHIIHH',1,b'\0'*6,len(ps),192,320,8,3)+b'\0'*12
        data=bytearray(header+struct.pack('>H',int(rle)))
        if rle:
            rows=[]
            for plane in ps:
                for y in range(192):
                    row=plane[y*320:(y+1)*320]
                    # PackBits literal packets, deliberately crossing 128-byte boundaries.
                    rows.append(b''.join(bytes([len(row[i:i+128])-1])+row[i:i+128] for i in range(0,320,128)))
            data+=b''.join(struct.pack('>H',len(row)) for row in rows)+b''.join(rows)
        else:
            data+=b''.join(ps)
        (folder/name).write_bytes(data)
    im.convert('RGB').save(folder/'06-opaque.jpg',quality=95)
    manifest={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in folder.iterdir() if p.is_file()}
    (folder/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    print(json.dumps(manifest),flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output',type=Path)
    make(parser.parse_args().output)
