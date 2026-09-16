"""Check direct-COM probe output against synthetic reference images. No registry changes."""
from pathlib import Path
import argparse
import json
from PIL import Image

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('output',type=Path)
p.add_argument('fixtures',type=Path)
args=p.parse_args()
out=args.output.resolve()
images={}
for meta_path in out.glob('*.json'):
    meta=json.loads(meta_path.read_text(encoding='utf-8'))
    if 'stride' not in meta:
        continue
    im=Image.frombytes('RGBA',(meta['width'],meta['height']),meta_path.with_suffix('.bgra').read_bytes(),
        'raw','BGRA',meta['stride'],1)
    images[meta_path.stem]=im
    im.save(meta_path.with_suffix('.png'))
assert len(images)==12
for size in [128,512]:
    reference=images[f'01-transparent.png-{size}'].tobytes()
    for name in ['02-static.webp','04-raw.psd','05-rle-extra.psd']:
        assert reference==images[f'{name}-{size}'].tobytes(),(name,size,'RGBA differs')
    animated=images[f'03-animated.webp-{size}'].tobytes()
    assert reference[3::4]==animated[3::4]
    assert all(reference[i:i+3]==animated[i:i+3] for i in range(0,len(reference),4) if reference[i+3])
original=Image.open(args.fixtures/'01-transparent.png').convert('RGBA')
assert images['01-transparent.png-512'].tobytes()==original.tobytes()
preview=images['05-rle-extra.psd-512']
background=Image.new('RGBA',preview.size,(235,235,235,255))
background.alpha_composite(preview)
background.convert('RGB').save(out/'preview-on-light.png')
result={'result':'PASS','thumbnail_outputs':12,'static_rgba_comparisons':6,
        'animated_visible_pixels_and_alpha_comparisons':2,'unscaled_png_matches_source':True,
        'note':'Animated WebP clears invisible RGB during frame composition; all visible RGB and alpha are identical.'}
(out/'verification.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result),flush=True)
