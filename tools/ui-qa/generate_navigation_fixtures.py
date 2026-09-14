"""Generate three navigation/blur fixtures in a NEW directory; requires Pillow."""
import argparse
from pathlib import Path
from PIL import Image, ImageDraw

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('output', type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
for number, label in ((1, 'first'), (2, 'middle'), (3, 'last')):
    image = Image.new('RGB', (1280, 860))
    pixels = image.load()
    for y in range(860):
        for x in range(1280):
            if x < 160 or x >= 1120:
                pixels[x, y] = (83, 108, 135) if (x // 4 + y // 4) % 2 else (210, 225, 238)
            else:
                pixels[x, y] = (int(200 + 24 * x / 1280), int(207 + 20 * y / 860), int(224 + 14 * x / 1280))
    ImageDraw.Draw(image).text((590, 410), f'FRAME {number}', fill=(55, 72, 95))
    image.save(args.output / f'0{number}_{label}.png')
