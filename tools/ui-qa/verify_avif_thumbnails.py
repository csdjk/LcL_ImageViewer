"""Validate direct-COM AVIF thumbnails against PNG reference thumbnails, without registry changes."""
from pathlib import Path
import argparse, json
from PIL import Image, ImageChops

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('output',type=Path)
args=parser.parse_args()
out=args.output.resolve()
results=[]
for name in ['alpha','opaque','animated']:
    for size in [128,512]:
        images=[]
        for filename in [f'{name}.avif-{size}',f'{name}-reference.png-{size}']:
            meta=json.loads((out/f'{filename}.json').read_text('utf-8'))
            im=Image.frombytes('RGBA',(meta['width'],meta['height']),(out/f'{filename}.bgra').read_bytes(),'raw','BGRA',meta['stride'],1)
            im.save(out/f'{filename}.png')
            images.append(im)
            if '.avif-' in filename:
                assert meta['alpha']==(2 if name=='alpha' else 1),(filename,meta)
        actual,reference=images
        assert actual.size==reference.size
        assert actual.getchannel('A').tobytes()==reference.getchannel('A').tobytes()
        visible=Image.new('RGBA',actual.size,(232,232,232,255))
        visible.alpha_composite(actual)
        expected=Image.new('RGBA',reference.size,(232,232,232,255))
        expected.alpha_composite(reference)
        peak=max(high for low,high in ImageChops.difference(visible,expected).getextrema())
        assert peak<=2,(name,size,peak)
        results.append({'name':name,'cx':size,'size':actual.size,'alpha_exact':True,'visible_peak_error':peak})
report={'result':'PASS','outputs':12,'avif_cases':6,'comparisons':results,'registry_modified':False}
(out/'verification.json').write_text(json.dumps(report,indent=2),'utf-8')
print(json.dumps(report),flush=True)
