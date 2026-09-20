"""Native Windows palette interaction and pixel checks; new fixtures and isolated profiles only."""
from pathlib import Path
from types import SimpleNamespace
import argparse
import json
from PIL import Image, ImageChops
import neumorphic_smoke as qa


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixtures = out / 'fixtures'
    fixtures.mkdir()
    im = Image.new('RGBA', (160, 96))
    for y in range(96):
        for x in range(160):
            im.putpixel((x, y), (220, 80, 45, [0, 64, 128, 192, 255][x // 32]))
    for name in ['01-alpha.png', '02-next.png']:
        im.save(fixtures / name)
    hashes = {p.name: qa.sha(p) for p in fixtures.iterdir()}
    bx = 611 if args.width == 880 else 902
    left = bx - 8  # popup x is button left; content has an 8pt inset
    actions = []
    def wait(seconds=.28): actions.append({'kind': 'wait', 'seconds': seconds})
    def click(x, y): actions.append({'kind': 'click', 'x': x, 'y': y}); wait()
    def key(code, modifiers=None): actions.append({'kind': 'key', 'code': code, 'modifiers': modifiers or []})
    def move(): actions.extend([{'kind': 'move', 'x': 30, 'y': 135}, {'kind': 'move', 'x': 35, 'y': 136}])
    def shot(name): move(); wait(.2); actions.append({'kind': 'shot', 'name': name})
    def open_palette():
        move(); wait(); click(bx, 34); click(left + 90, 276); wait()
    def type_hex(text):
        click(left + 150, 300); key('A', ['ctrl'])
        for char in text: key(char)
        wait()
    key('0'); key('5'); shot('base'); key('4'); shot('base-alpha'); key('5')
    actions.append({'kind': 'assert-topmost', 'value': True})
    open_palette(); shot('initial-black'); wait(1.4); shot('palette-held')
    click(left + 152, 269); click(left + 200, 123); shot('blue-live')
    actions.append({'kind': 'drag', 'x': left + 200, 'y': 123, 'dx': -95, 'dy': 60, 'expect_window_delta': [0, 0, 0, 0]})
    shot('sv-drag')
    actions.append({'kind': 'drag', 'x': left + 152, 'y': 269, 'dx': -76, 'dy': 0, 'expect_window_delta': [0, 0, 0, 0]})
    shot('hue-drag')
    # Exact swatch choices, then recover a chromatic color from black and white.
    click(left + 11, 357); shot('white-swatch')
    click(left + 70, 357); shot('black-swatch')
    click(left + 190, 269); click(left + 175, 125); shot('recovered-color')
    type_hex('3467CD'); shot('hex-live')
    type_hex('GGGGGG'); shot('hex-invalid')
    click(left + 100, 396); shot('invalid-cannot-apply')
    type_hex('8A56CC'); shot('palette-selected')
    click(left + 100, 396); shot('applied')
    key('4'); shot('selected-alpha'); key('5')
    open_palette(); shot('reopened')
    # Back to presets without dismissing; verify the old white/black/gray entry still works.
    click(left + 16, 82); shot('back-to-presets')
    click(left + 90, 208); shot('gray-preset')
    open_palette(); type_hex('8A56CC'); key(13); wait(); shot('enter-applied')
    open_palette(); key(27); wait(); shot('escape-dismissed')
    actions.append({'kind': 'assert-topmost', 'value': True})
    wait(1.5); actions.append({'kind': 'shot', 'name': 'toolbar-hidden'}); shot('toolbar-restored')
    path = out / 'actions.json'
    path.write_text(json.dumps(actions, indent=2), encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary, output=out / 'viewer', input=fixtures / '01-alpha.png',
        theme=args.theme, width=args.width, height=args.height, commit=args.commit, actions=path,
        isolated_profile=True, seed_preferences={'iv-background-color': '#000000', 'iv-checkerboard': 'off', 'iv-always-on-top': 'on'}))
    meta = json.loads((out / 'viewer/run.json').read_text(encoding='utf-8'))
    def image(name): return Image.open(out / 'viewer' / f'{name}.png').convert('RGB')
    def rgb(name): return image(name).getpixel((24, 145))
    def peak(a, b): return max(hi for lo, hi in ImageChops.difference(a, b).getextrema())
    for name, expected in [('base', (0, 0, 0)), ('white-swatch', (255, 255, 255)), ('black-swatch', (0, 0, 0)),
        ('hex-live', (52, 103, 205)), ('hex-invalid', (52, 103, 205)), ('invalid-cannot-apply', (52, 103, 205)),
        ('applied', (138, 86, 204)), ('enter-applied', (138, 86, 204)), ('escape-dismissed', (138, 86, 204)), ('gray-preset', (128, 128, 128))]:
        assert max(abs(a - b) for a, b in zip(rgb(name), expected)) <= 1, (name, rgb(name), expected)
    blue = rgb('blue-live'); assert blue[2] > 200 and blue[2] > blue[0] * 3, blue
    assert rgb('sv-drag') != rgb('blue-live'), 'SV dragging did not update preview'
    assert rgb('hue-drag') != rgb('sv-drag'), 'Hue dragging did not update preview'
    assert max(rgb('recovered-color')) - min(rgb('recovered-color')) > 100, 'Black state lost its hue'
    # Solid image pixels and the decoded alpha channel are invariant under background changes.
    opaque = (args.width//2 + 55, args.height//2 - 30, args.width//2 + 72, args.height//2 + 30)
    for name in ['blue-live', 'hex-live', 'applied', 'gray-preset']:
        assert peak(image('base').crop(opaque), image(name).crop(opaque)) == 0, (name, 'image pixels altered')
    area = (args.width//2 - 78, args.height//2 - 46, args.width//2 + 78, args.height//2 + 46)
    assert peak(image('base-alpha').crop(area), image('selected-alpha').crop(area)) == 0
    # A full color area, not another slider-only panel; assert it remains open after invalid Apply.
    for name in ['initial-black', 'palette-held', 'invalid-cannot-apply', 'palette-selected', 'reopened']:
        colors = image(name).crop((int(left)+10, 110, int(left)+215, 245)).getcolors(40000)
        assert colors and len(colors) > 1000, (name, 'color palette missing')
    assert rgb('toolbar-hidden') == (138, 86, 204)
    assert image('toolbar-hidden').getpixel((args.width//2, 20)) == (138, 86, 204)
    assert image('toolbar-restored').getpixel((args.width//2, 20)) != (138, 86, 204)
    assert meta['preferences_restored']
    saved = meta['qa_saved_settings']
    assert saved['iv-background-color'] == '#8A56CC' and saved['iv-checkerboard'] == 'off'
    assert saved['iv-theme'] == args.theme and saved['iv-always-on-top'] == 'on'
    assert hashes == {p.name: qa.sha(p) for p in fixtures.iterdir()}
    for name in ['hex-live', 'hex-invalid', 'applied']:
        info = json.loads((out / 'viewer' / f'{name}.json').read_text(encoding='utf-8'))
        assert info['window_title'].startswith('01-alpha.png'), 'Typing triggered image navigation'
    # Fresh-process round trip of the actual saved value, not a hardcoded assumed preference.
    restore = [{'kind': 'key', 'code': '0'}, {'kind': 'assert-topmost', 'value': True},
        {'kind': 'move', 'x': 30, 'y': 135}, {'kind': 'move', 'x': 35, 'y': 136},
        {'kind': 'wait', 'seconds': .3}, {'kind': 'shot', 'name': 'restored'}]
    p = out / 'restart.json'; p.write_text(json.dumps(restore), encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary, output=out / 'restart', input=fixtures / '01-alpha.png',
        theme=args.theme, width=args.width, height=args.height, commit=args.commit, actions=p, isolated_profile=True,
        seed_preferences={k: saved[k] for k in ['iv-background-color', 'iv-checkerboard', 'iv-always-on-top']}))
    restored = Image.open(out / 'restart/restored.png').convert('RGB')
    assert restored.getpixel((24, 145)) == (138, 86, 204)
    restart = json.loads((out / 'restart/run.json').read_text(encoding='utf-8')); assert restart['preferences_restored']
    report = {'result': 'PASS; visual review required', 'commit': args.commit, 'binary_sha256': qa.sha(args.binary),
        'theme': args.theme, 'client': [args.width, args.height], 'captures': len(meta['captures']) + len(restart['captures']),
        'sv_hue_click_drag': True, 'live_hex_and_invalid_guard': True, 'swatches_and_black_recovery': True,
        'background_and_alpha_pixels_verified': True, 'pin_preserved': True, 'enter_escape_back': True,
        'persistence_round_trip': True, 'preferences_and_source_unchanged': True}
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['binary', 'output']: p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--commit', required=True)
    p.add_argument('--theme', choices=['dark', 'light'], default='dark')
    p.add_argument('--width', type=int, choices=[880, 1280], default=880)
    p.add_argument('--height', type=int, default=560)
    run(p.parse_args())
