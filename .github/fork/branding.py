"""Xnes3D+ branding, applied at build time (Android APK workflow) so nixxou's files stay as
they are in the repository and the weekly catch-up merges cleanly.

- the app id and names: io.github.gin649.zaxnes, "Xnes3D+", "Xnes3D+ profiles"
- the launcher icon (.github/fork/res, the voxel plus of the logo), compiled by the workflow and
  linked by build/package-android.py through ZAXNES_EXTRA_RES
- the colours of the handheld interface (the one Android uses): the NES look, charcoal, black
  and the console's red

Each change checks that the text it replaces is there; if Zaxnes changes it, the build stops
here, saying which, instead of quietly building without the branding.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RUST = ROOT / 'rust'


def replace(path, old, new, count=1):
    text = path.read_text(encoding='utf-8')
    found = text.count(old)
    if found < count:
        sys.exit(f'branding: "{old}" is no longer in {path.relative_to(ROOT)}')
    path.write_text(text.replace(old, new), encoding='utf-8')


# App id, names, icon
manifest = RUST / 'build' / 'android' / 'AndroidManifest.xml'
replace(manifest, 'package="io.github.nixxou.zaxnes"', 'package="io.github.gin649.zaxnes"')
replace(manifest, 'android:label="Zaxnes"', 'android:label="Xnes3D+"', count=2)
replace(manifest, 'android:label="Zaxnes profiles"', 'android:label="Xnes3D+ profiles"')
replace(manifest, '<application', '<application\n        android:icon="@mipmap/ic_launcher"')

# The packaging links the compiled icon (ZAXNES_EXTRA_RES) with the manifest
script = RUST / 'build' / 'package-android.py'
replace(script, "run([tool(tools, 'aapt2'), 'link', '-o', linked,",
        "run([tool(tools, 'aapt2'), 'link', '-o', linked, "
        "*([os.environ['ZAXNES_EXTRA_RES']] if os.environ.get('ZAXNES_EXTRA_RES') else []),")

# Handheld interface colours: the NES look. A charcoal screen, black bands with the console's red
# stripe (drawn by draw.rs), a red focus bar with white text. The colours that mean something (a
# game with a profile, save and load, warnings) stay as they are.
draw = RUST / 'crates' / 'zax-app' / 'src' / 'ui' / 'handheld' / 'draw.rs'
palette = {
    'BACKGROUND': '[0.11, 0.11, 0.12, 1.0]',
    'PANEL': '[0.14, 0.14, 0.15, 0.97]',
    'TEXT': '[0.93, 0.93, 0.91, 1.0]',
    'DIM': '[0.68, 0.68, 0.68, 1.0]',
    'FOCUS': '[0.85, 0.09, 0.15, 1.0]',
    'ON_FOCUS': '[1.0, 1.0, 1.0, 1.0]',
    'PROFILE_ON_FOCUS': '[1.0, 0.92, 0.72, 1.0]',
    'GAME_OWN_ON_FOCUS': '[0.85, 1.0, 1.0, 1.0]',
    'BAR': '[0.04, 0.04, 0.04, 1.0]',
    'GROUP_TITLE': '[0.92, 0.38, 0.38, 1.0]',
}
text = draw.read_text(encoding='utf-8')
for name, value in palette.items():
    pattern = re.compile(r'^const ' + name + r': \[f32; 4\] = \[[^\]]*\];', re.M)
    if not pattern.search(text):
        sys.exit(f'branding: the colour {name} is no longer in {draw.relative_to(ROOT)}')
    text = pattern.sub(f'const {name}: [f32; 4] = {value};', text, count=1)
draw.write_text(text, encoding='utf-8')

print('Xnes3D+ branding applied.')
