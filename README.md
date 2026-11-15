# OpenCorel — independent Rust vector editor

[Русская версия](README.ru.md) · [Руководство по работе](docs/USER_GUIDE_RU.md) · [Format matrix](docs/FORMAT_SUPPORT.md) · [KDR specification](docs/KDR_SPEC.md) · [Contributing](CONTRIBUTING.md) · [QA criteria](docs/QA_MATRIX.md)

**Alpha v0.10 preview — NOT a fully compatible replacement for CorelDRAW.** Designed for posters, maps and large advertising sheets, currently targeting Windows 10/11 x64 and Linux x64. Windows 7 is not tested.

- v0.10: **Boolean operations** for two selected, editable closed filled figures: union, A minus B, intersection, and XOR. Results remain native compound KDR contours, including disconnected shapes and holes. Uses bounded 0.1mm curve flattening, refuses unsupported/empty/error cases without changing the source, and offers Undo.
- v0.9: interactive **F10 node-editing tool** for polylines, Bézier curves and individual contours: mouse dragging of anchors and handles, double-click to insert nodes, Del to remove anchors, arrow-key nudging; undo and native KDR persistence.
- [20-iteration engineering plan](docs/ROADMAP_20_ITERATIONS.md) defines ordered work, quality gates and transparent progress.
- v0.8: modern slate/teal theme, persistent names, searchable object list, group/ungroup (Ctrl+G / Ctrl+Shift+G), linked group selection and movement.
- v0.8: exact X/Y/width/height properties in millimetres, configurable snapping grid, multi-object page centering and correct z-order changes for grouped items.
- Task-focused professional workspace with **labelled tools**, contextual how-to instructions, first-run onboarding, real interactive tutorial layout and honest format/print limitations.
- New **line and hexagon tools**, four-corner mouse resizing with Shift ratio lock, 1/10 mm arrow-key nudging, 90-degree rotation/mirroring of supported geometry.
- **Internal** Ctrl+C/Ctrl+X/Ctrl+V clipboard, Ctrl+D clones and object format preservation. Clipboard does not exchange data with third-party applications.
- Familiar page rulers, toolbox, layers, color palette, undo/redo, Bézier paths and grid snapping.
- v0.6: explicit conversion of editable text to true compound cubic Bézier outlines using installed fonts and Unicode shaping; holes in glyphs preserved.
- v0.6: SVG even-odd compound paths re-import as editable contours; batch CLI text outlining.
- v0.5: independently editable per-text font family, boldness (100–900), italic, paragraph alignment, letter spacing and multiple lines; exported as SVG editable text where supported.
- v0.5: interactive layout preflight for off-page art, image DPI, font portability and RGB/print limitations; non-exported safe-area guide and A0/A2/large-format presets.
- **New `.kdr` (Kodi Draw)** native compressed editable project: versioned manifest, checksum and SVG preview; backward-compatible import of old `.ocd`.
- SVG and SVGZ import with a documented editable subset.
- Unsupported complex vector details can be preserved as one placed SVG artwork object, not individual editable entities.
- Experimental local LibreOffice/Poppler import adapters for CDR, CMX, PDF, AI, EPS, ODG, DXF, WMF/EMF, WPG, Visio, etc.
- PNG/JPEG photos and logos placed inside vector layouts; support WebP/GIF/BMP/TIFF input via normalization.
- Multi-object Shift selection and Ctrl+A; align edges and distribute centers; group dragging and edits.
- RGB PNG/JPEG customer proofs at configurable 36–300 DPI (maximum 24 megapixels, 10k pixels per side).
- Memory-bounded history, thumbnail caches and geometry-only editing of embedded artwork.
- Basic SVG export and RGB PDF proof export. **No CDR export, no PDF/X or CMYK, no verified Windows 7 support.**

## Build

```sh
git clone https://github.com/konstive-collab/OpenCorel.git
cd OpenCorel
cargo test --workspace
cargo run --release -p opencorel-desktop
```

## CLI examples

```sh
cargo run --release -p opencorel-cli -- new poster.kdr
cargo run --release -p opencorel-cli -- convert drawing.svg drawing.kdr
cargo run --release -p opencorel-cli -- convert source.cdr source.kdr
cargo run --release -p opencorel-cli -- export-pdf poster.kdr proof.pdf
cargo run --release -p opencorel-cli -- outline-text poster.kdr outlined.kdr
```

For CDR/CMX and many legacy formats, the application needs a separate installation of **LibreOffice** and often **Poppler pdftocairo**. You may configure `OPENCOREL_SOFFICE` to point to the LibreOffice binary.

Import adapters are **experimental, potentially lossy, and usually single-page**. A listed extension does not mean every version is supported. **Do not delete original CDR/PDF/AI files or replace production artwork without visual and print checks.** See [the format matrix](docs/FORMAT_SUPPORT.md).

MIT-licensed independent project, not affiliated with Corel Corporation. OpenCorel is a provisional working name subject to trademark review.
