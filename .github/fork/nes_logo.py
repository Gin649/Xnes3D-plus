"""Draws the Xnes3D+ logo (README) and app icon in an NES look: pixel letters, console grey,
black band and red stripe. Run from the repository root."""
import subprocess, sys
from pathlib import Path

FONT = {  # 5 x 7 pixel letters
 'Z': ["11111","00001","00010","00100","01000","10000","11111"],
 'A': ["01110","10001","10001","11111","10001","10001","10001"],
 'X': ["10001","10001","01010","00100","01010","10001","10001"],
 'N': ["10001","11001","10101","10011","10001","10001","10001"],
 'E': ["11111","10000","10000","11110","10000","10000","11111"],
 'S': ["01111","10000","10000","01110","00001","00001","11110"],
 'P': ["11110","10001","10001","11110","10000","10000","10000"],
 'L': ["10000","10000","10000","10000","10000","10000","11111"],
 'U': ["10001","10001","10001","10001","10001","10001","01110"],
 'D': ["11110","10001","10001","10001","10001","10001","11110"],
 '3': ["11110","00001","00001","01110","00001","00001","11110"],
 '+': ["00000","00100","00100","11111","00100","00100","00000"],
 ' ': ["000","000","000","000","000","000","000"],
}
GREY, LIGHT, DARK, BLACK, RED = "#C9C9C6", "#E4E4E0", "#5C5C5E", "#151515", "#D7192A"

def word(text, x, y, px, fill, shadow=None, gap=1):
    out = []
    for ch in text:
        rows = FONT[ch]
        for r, row in enumerate(rows):
            for c, bit in enumerate(row):
                if bit == "1":
                    if shadow:
                        out.append(f'<rect x="{x+c*px+px*0.5:g}" y="{y+r*px+px*0.5:g}" width="{px:g}" height="{px:g}" fill="{shadow}"/>')
        for r, row in enumerate(rows):
            for c, bit in enumerate(row):
                if bit == "1":
                    out.append(f'<rect x="{x+c*px:g}" y="{y+r*px:g}" width="{px:g}" height="{px:g}" fill="{fill}"/>')
        x += (len(rows[0]) + gap) * px
    return out, x

def width(text, px, gap=1):
    return sum((len(FONT[ch][0]) + gap) * px for ch in text) - gap * px

def logo():
    px = 8
    w1, w2 = width("XNES3D", px), width("+", px)
    pad = 28
    W = pad * 2 + w1 + px * 2 + w2
    H = 7 * px + pad * 2 + 24
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" shape-rendering="crispEdges">',
             f'<rect x="0" y="0" width="{W}" height="{H}" rx="10" fill="{BLACK}"/>',
             f'<rect x="0" y="{H-18}" width="{W}" height="6" fill="{RED}"/>']
    a, x = word("XNES3D", pad, pad, px, LIGHT, shadow=DARK)
    b, _ = word("+", x + px, pad, px, RED, shadow="#6E0D15")
    parts += a + b + ['</svg>']
    return "\n".join(parts)

def icon():
    S = 240
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {S} {S}" width="{S}" height="{S}" shape-rendering="crispEdges">',
             f'<rect width="{S}" height="{S}" fill="{GREY}"/>',
             f'<rect y="0" width="{S}" height="78" fill="{BLACK}"/>',
             f'<rect y="78" width="{S}" height="8" fill="{RED}"/>',
             # ridges, as on the console's lid
             *[f'<rect x="20" y="{y}" width="{S-40}" height="4" fill="#2A2A2A"/>' for y in (18, 34, 50)],
             ]
    px = 16
    total = 5 * px * 2 + px
    left = (S - total) / 2
    top = 86 + (S - 86 - 7 * px) / 2
    z, x = word("X", left, top, px, BLACK, shadow="#8E8E8B")
    plus, _ = word("+", x, top, px, RED, shadow="#7A0E18", gap=0)
    parts += z + plus + ['</svg>']
    return "\n".join(parts)

root = Path(sys.argv[1])
(root / ".github/xnes3d-logo.svg").write_text(logo())
(root / ".github/fork/icon.svg").write_text(icon())
for folder, size in {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}.items():
    out = root / f".github/fork/res/mipmap-{folder}/ic_launcher.png"
    subprocess.run(["rsvg-convert", "-w", str(size), "-h", str(size), "-o", str(out), str(root / ".github/fork/icon.svg")], check=True)
subprocess.run(["rsvg-convert", "-o", "/tmp/logo.png", str(root / ".github/xnes3d-logo.svg")], check=True)
subprocess.run(["rsvg-convert", "-w", "192", "-o", "/tmp/icon.png", str(root / ".github/fork/icon.svg")], check=True)
