#!/usr/bin/env python3
"""Bundles the deck into one self-contained HTML file (fonts, logo, data and code inlined), like the site's
one-file version: it opens from disk without a server or a network. Run: python3 deck/build.py [name.html]
"""
import base64
import pathlib
import sys

DECK = pathlib.Path(__file__).resolve().parent
REPO = DECK.parent
OUT = DECK / 'dist' / (sys.argv[1] if len(sys.argv) > 1 else 'turkish-draughts-a4.html')

# fontsource subsets (latin covers English and ü, ç; latin-ext adds ş, ğ, ı)
RANGES = {
    'latin': 'U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0304,U+0308,U+0329,U+2000-206F,'
             'U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD',
    'latin-ext': 'U+0100-02BA,U+02BD-02C5,U+02C7-02CC,U+02CE-02D7,U+02DD-02FF,U+0304,U+0308,U+0329,U+1D00-1DBF,'
                 'U+1E00-1E9F,U+1EF2-1EFF,U+2020,U+20A0-20AB,U+20AD-20C0,U+2113,U+2C60-2C7F,U+A720-A7FF',
}
FONTS = [  # css family, file stem, weight, style
    ('Young Serif', 'young-serif', 400, 'normal'),
    ('Instrument Sans', 'instrument-sans', 400, 'normal'),
    ('Instrument Sans', 'instrument-sans', 500, 'normal'),
    ('Instrument Sans', 'instrument-sans', 600, 'normal'),
    ('STIX Two Text', 'stix-two-text', 400, 'normal'),
    ('STIX Two Text', 'stix-two-text', 400, 'italic'),
    ('JetBrains Mono', 'jetbrains-mono', 400, 'normal'),
    ('JetBrains Mono', 'jetbrains-mono', 600, 'normal'),
]
SCRIPTS = [DECK / 'src' / 'data.js', REPO / 'reference' / 'js' / 'engine.js', DECK / 'src' / 'core.js',
           DECK / 'src' / 'scenes.js', DECK / 'src' / 'player.js']


def font_faces():
    rules = []
    for fam, stem, w, st in FONTS:
        for sub, rng in RANGES.items():
            b64 = base64.b64encode((DECK / 'fonts' / f'{stem}-{sub}-{w}-{st}.woff2').read_bytes()).decode()
            rules.append(f'@font-face{{font-family:"{fam}";font-style:{st};font-weight:{w};font-display:block;'
                         f'src:url(data:font/woff2;base64,{b64}) format("woff2");unicode-range:{rng};}}')
    return '\n'.join(rules)


def main():
    css = (DECK / 'src' / 'style.css').read_text()
    logo = base64.b64encode((DECK / 'img' / 'eightbits_logo.png').read_bytes()).decode()
    parts = [f'window.LOGO_SRC = "data:image/png;base64,{logo}";'] + [p.read_text() for p in SCRIPTS]
    parts = [p.replace('</script', '<\\/script') for p in parts]  # a script may not contain its own end tag
    html = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Turkish Draughts: Search, Measured</title>
<meta name="description" content="Introduction to AI, Assignment 4: our Turkish draughts agent, adversarial search and learning, measured on its own games. Team Eight Bits, pair 42: Hüseyin Bozan, Murat Çevlik.">
<style>
{font_faces()}
{css}
</style>
</head>
<body>
<canvas id="cv" tabindex="0" aria-label="Presentation; use the arrow keys to move between steps"></canvas>
<div id="bar"></div>
<div id="hint">Press → to advance, F for full screen, N for speaker notes</div>
<div id="notes" aria-live="polite"></div>
''' + ''.join(f'<script>\n{p}\n</script>\n' for p in parts) + '</body>\n</html>\n'
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(html)
    print(f'wrote {OUT.relative_to(REPO)} ({OUT.stat().st_size / 1024:.0f} KB)')


if __name__ == '__main__':
    main()
