"""Compress current native screenshots for README and record their exact provenance.
Only cropping, resizing, outside captions and GIF encoding are used; no UI retouching.
"""
from __future__ import annotations
import argparse, hashlib, json
from pathlib import Path
from PIL import Image, ImageChops, ImageDraw, ImageFont


def run(args):
    root=Path(__file__).resolve().parents[2];source=args.source.resolve()
    target=root/'docs/screenshots/features/v0.5.0';target.mkdir(parents=True,exist_ok=False)
    gallery=json.loads((source/'gallery.json').read_text('utf-8'));records={}
    font_path=Path('C:/Windows/Fonts/msyh.ttc')
    font=ImageFont.truetype(str(font_path),18);small=ImageFont.truetype(str(font_path),15)
    def read(folder,name):return Image.open(source/folder/(name+'.png')).convert('RGB')
    def source_record(folder,name,crop=None):
        p=source/folder/(name+'.png');m=json.loads(p.with_suffix('.json').read_text('utf-8'))
        assert m['commit']==gallery['capture_commit'] and m['dpi']==96
        return {'capture':folder+'/'+p.name,'capture_sha256':hashlib.sha256(p.read_bytes()).hexdigest(),
                'binary_sha256':m['binary_sha256'],'logical_client':m['logical_client'],'crop':crop}
    def save(name,image,inputs):
        image.save(target/name,quality=90,subsampling=0,optimize=True)
        records[name]={'sources':inputs,'bytes':(target/name).stat().st_size,'size':image.size}
    def single(name,folder,state,crop=None):
        im=read(folder,state)
        if crop:im=im.crop(crop)
        save(name,im,[source_record(folder,state,crop)])
    def panel(name,specs,columns,tile=(440,280)):
        rows=(len(specs)+columns-1)//columns;gap=12;caption=36
        canvas=Image.new('RGB',(columns*tile[0]+gap*(columns-1),rows*(tile[1]+caption)+gap*(rows-1)),(242,245,250));inputs=[]
        for i,(folder,state,label,crop) in enumerate(specs):
            im=read(folder,state)
            if crop:im=im.crop(crop)
            im=im.resize(tile,Image.Resampling.LANCZOS);x=(i%columns)*(tile[0]+gap);y=(i//columns)*(tile[1]+caption+gap)
            ImageDraw.Draw(canvas).text((x+12,y+7),label,font=font,fill=(40,52,70));canvas.paste(im,(x,y+caption));inputs.append(source_record(folder,state,crop))
        save(name,canvas,inputs)
    panel('rgba-channels.jpg',[( 'rgba-dark',s,label,[208,64,672,504]) for s,label in [('rgba','完整 RGBA · 5 / C'),('red','红通道 R · 1'),('green','绿通道 G · 2'),('blue','蓝通道 B · 3'),('alpha','透明通道 Alpha · 4'),('rgb-opaque','忽略透明度 RGB · O')]],3,(300,285))
    single('pixel-inspection.jpg','probe-dark','pixel-value')
    single('folder-browsing.jpg','browse-light','subfolder')
    panel('zoom-and-bounds.jpg',[('view-dark','bounds','高亮完整图片边界',None),('view-dark','zoom-nearest','最近邻采样 · 像素检查',None)],2)
    panel('mip-and-hdr.jpg',[('mip-dark','mip-1','切换图片自带的 Mipmap',None),('hdr-dark','hdr','HDR 曝光调节',None)],2)
    single('always-on-top.jpg','pin-light','pinned')
    single('color-palette.jpg','palette-dark','palette')
    panel('themes-and-settings.jpg',[('theme-dark','settings','深色主题与设置',None),('theme-light','settings','浅色主题与设置',None)],2)
    panel('file-actions.jpg',[('file-light','file-menu','复制路径 / 打开目录等操作',None),('file-light','delete-confirm','删除前确认，可取消',None)],2)
    # Sequential frames were captured from a running native viewer, including real pause/steps.
    frames_dir=source/'animation-dark';roi=(380,235,900,625)
    def patch(n):return read('animation-dark',n).crop(roi).tobytes()
    play=[f'play-{i:03}' for i in range(28)];pause=[f'pause-{i:03}' for i in range(12)];step=[f'step-{i}' for i in range(4)];resume=[f'resume-{i:03}' for i in range(16)]
    assert len({patch(n) for n in play})>=8,'Animation is not visibly playing'
    assert len({patch(n) for n in pause})==1,'Pause did not hold the displayed frame'
    assert len({patch(n) for n in step})==4,'Step controls did not change the frame'
    assert len({patch(n) for n in resume})>=4,'Resume did not restart animation'
    images=[];durations=[];inputs=[]
    for names,label,duration in [(play,'自动播放 · 按源文件帧时长循环',100),(pause,'Space 暂停 · 停留在当前帧',100),(step,'逐帧查看 · 按逗号 / 句号切换',400),(resume,'Space 继续播放',100)]:
        for n in names:
            im=read('animation-dark',n).resize((880,591),Image.Resampling.LANCZOS)
            card=Image.new('RGB',(880,623),(242,245,250));card.paste(im,(0,32));ImageDraw.Draw(card).text((14,5),label,font=font,fill=(40,52,70))
            images.append(card.quantize(colors=128,method=Image.Quantize.MEDIANCUT));durations.append(duration);inputs.append(source_record('animation-dark',n))
    gif=target/'animation-controls.gif';images[0].save(gif,save_all=True,append_images=images[1:],duration=durations,loop=0,optimize=True,disposal=1)
    with Image.open(gif) as im:assert im.n_frames>=40
    records[gif.name]={'sources':inputs,'bytes':gif.stat().st_size,'size':[880,623],'recorded_play_pause_step_resume':True,'duration_ms':sum(durations)}
    # Keep the existing overview paths accurate as well.
    for theme in ['dark','light']:
        read('theme-'+theme,'theme').save(root/f'docs/screenshot-{theme}.jpg',quality=91,subsampling=0,optimize=True)
    manifest={'version':'0.5.0','capture_source_commit':gallery['capture_commit'],'method':'Real Windows client screenshots; optional crops and outside captions, no UI redraw','illustrations':records}
    (target/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),'utf-8')
    print(json.dumps({'output':str(target),'files':[(n,v['bytes']) for n,v in records.items()],'animation_verified':True},ensure_ascii=False),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--source',type=Path,required=True);run(p.parse_args())
