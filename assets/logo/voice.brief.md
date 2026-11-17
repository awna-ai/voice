# Logo: provenance

Two files, and the app icons made from the first:

- `voice.svg`: the logo, on its navy tile. Use it anywhere, on light or dark.
- `voice-mark.svg`: the bars alone, with no tile, for dark surfaces only. Against white its sky
  blue and amber reach about 2.1:1, under the 3:1 that graphics need; against `#0F172A` or
  `#0D1117` they reach 8.3:1 or more.
- `builder/app/icons/`: the app's icons, made from `voice.svg` (below). They replace the PB7.1
  placeholder, a white speech bubble on blue.

It belongs to one family with Flightdeck's radar and Radio's signal: a slate navy tile, sky-blue
shapes, one amber accent.

- **Chosen** on 2026-11-16 from round 1. The montage showed four concepts beside the
  Flightdeck and Radio icons at 160 px and 32 px, on light and dark: a speech bubble with an amber
  dot, this waveform, a head in profile speaking, and two overlapping speech bubbles. The waveform
  was picked.
- **Generator:** Recraft, model `recraftv4_1_vector`, 1:1, n=1.
- **Prompt:**
  > Minimal geometric app icon symbol. One rounded-square tile, solid slate navy (#0F172A),
  > transparent background outside the tile. Centred on the tile, well inside its edges: an audio
  > waveform made of five thick vertical sky-blue (#38BDF8) bars with fully rounded ends, short at
  > the edges and tallest in the middle, and the middle bar is solid amber (#F59E0B) instead of
  > blue, all of it fitting in the middle half of the tile. Flat vector, bold thick shapes, few
  > elements, no text, no hairlines, wide empty margin around the symbol, legible at 32 px.
- **Palette:** slate navy `#0F172A` (tile), sky blue `#38BDF8` (bars), amber `#F59E0B` (middle
  bar), the same three as Radio.

## Derivation

1. Remove the C2PA metadata and the white canvas path that Recraft adds outside the tile.
2. Map each fill to the nearest palette hex: the generator returned near misses
   (`rgb(34,162,243)` for the sky blue, `rgb(12,24,36)` for the navy, `rgb(254,182,83)` for the
   amber).
3. Crop the viewBox to the tile, square (`599 599 3167 3167`), and round path coordinates to whole
   units.
4. Scale the bars by 0.82 about the tile's centre, baked into the coordinates. As generated they
   spanned 78 % of the tile's width; scaled, they span 64 % by 56 %, the same as Radio's arcs
   (62 % by 56 %), so the family's icons sit at one size side by side.
5. Add `role="img"` with a `<title>`.
6. `voice-mark.svg`: drop the tile path, then crop the viewBox square around the bars with a margin
   of a twelfth of their size (`1004 1000 2354 2354`).
7. `builder/app/icons/icon.svg`: the paths of `voice.svg` on a 1024 px canvas, the tile scaled to
   824 px and centred (Apple's icon grid, as Flightdeck's icon is). Then
   `pnpm -C builder/ui tauri icon ../app/icons/icon.svg -o ../app/icons`, keeping only the five
   files `tauri.conf.json` lists and deleting the rest it writes.

## Checked

`voice.svg` and `voice-mark.svg` at 128 px and 32 px, on white and on a dark page, beside the
Flightdeck and Radio icons: the five bars stay separate at 32 px. `icon.icns` holds every size from
16 px to 1024 px; `icon.png`, `128x128.png` and `32x32.png` viewed at 256, 128 and 32 px.
