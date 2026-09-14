"""Read-only checks for real side-navigation captures (Pillow required).

Example: python tools/ui-qa/side_navigation_checks.py --shots ui-verify-shots
This does not control the desktop, modify viewer preferences, or create captures.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops, ImageStat


def max_difference(a: Image.Image, b: Image.Image, roi: tuple[int, int, int, int]) -> float:
    return max(ImageStat.Stat(ImageChops.difference(a.crop(roi), b.crop(roi))).mean)


def check_run(folder: Path, width: int, height: int) -> dict:
    run = json.loads((folder / 'run.json').read_text(encoding='utf-8'))
    assert run['isolated_profile'] and run['preferences_restored'], 'Unsafe preference isolation'
    assert len(run['captures']) == 16, 'Incomplete capture matrix'
    meta = {Path(n).stem: json.loads((folder / Path(n).with_suffix('.json')).read_text(encoding='utf-8'))
            for n in run['captures']}
    for state, value in meta.items():
        assert value['logical_client'] == [width, height], (state, 'Wrong size')
        assert value['dpi'] == 96, 'These pixel checks require recorded DPI96 captures'
    for state, expected in {
        'default': '02_middle.png',
        'first-prev-disabled': '01_first.png',
        'first-prev-noop': '01_first.png',
        'middle-after-next': '02_middle.png',
        'last-next-disabled': '03_last.png',
        'last-next-noop': '03_last.png',
        'keyboard-previous': '02_middle.png',
    }.items():
        assert meta[state]['window_title'].startswith(expected), (state, meta[state]['window_title'])
    # Long press can cancel a click in egui. Compare equal inputs rather than assuming
    # that releasing a long-held button must activate navigation.
    assert meta['before-timeout']['window_title'] == meta['hidden']['window_title'] == meta['restored']['window_title']
    before = Image.open(folder / 'before-timeout.png').convert('RGB')
    hidden = Image.open(folder / 'hidden.png').convert('RGB')
    restored = Image.open(folder / 'restored.png').convert('RGB')
    regions = [(160, 14, width - 160, 55),
               (18, height // 2 - 32, 66, height // 2 + 32),
               (width - 66, height // 2 - 32, width - 18, height // 2 + 32)]
    for roi in regions:
        assert max_difference(before, hidden, roi) > 3, ('Did not hide', roi)
        assert max_difference(before, restored, roi) < 0.03, ('Did not restore', roi)
    image_roi = (170, 90, width - 170, height - 90)
    image_error = max_difference(before, hidden, image_roi)
    assert image_error == 0, 'UI effect changed the unobstructed image interior'
    default = Image.open(folder / 'default.png').convert('RGB')
    glass_body = (width - 33, height // 2 - 19, width - 25, height // 2 + 19)
    visible_variance = sum(ImageStat.Stat(default.crop(glass_body)).var)
    sharp_variance = sum(ImageStat.Stat(hidden.crop(glass_body)).var)
    assert sharp_variance > 20, 'Fixture does not have sufficient detail at the button'
    ratio = visible_variance / sharp_variance
    assert ratio < 0.05, ('Glass failed to soften the high-frequency fixture', ratio)
    return {'run': folder.name, 'captures': len(run['captures']),
            'binary_sha256': run['binary_sha256'], 'commit': run['commit'],
            'image_interior_error': image_error, 'glass_body_variance_ratio': ratio,
            'navigation_and_hide_restore': 'PASS'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--shots', type=Path, required=True)
    parser.add_argument('--prefix', default='side-glass-final-')
    args = parser.parse_args()
    results = [check_run(args.shots / f'{args.prefix}{theme}-{width}', width, height)
               for theme in ('light', 'dark') for width, height in ((1280, 860), (880, 560))]
    print(json.dumps(results, ensure_ascii=False, indent=2))
