# OpenCorel — roadmap and status

Goal: fast, legally independent vector graphics editor for advertising, posters and maps, especially on older Windows/Linux workstations.

## Delivered in source (automated CI; manual acceptance still pending)
- **v0.1**: millimetre-based document engine, desktop GUI/CLI, OCD, SVG export
- **v0.2**: Bézier paths, layers, grid/snap, rulers, palette, undo/redo, native file pickers, PDF RGB proof
- **v0.3**: KDR ZIP format with checksum, CDR/CMX/AI/PDF conversion adapters, partial SVG editability
- **v0.4**: embedded PNG/JPEG photos, WebP/GIF/BMP/TIFF import, cached image previews, PNG/JPEG RGB proofs, bounded undo history, multi-select, alignment and distribution
- **v0.5**: text formatting & multiline SVG, layout preflight checks, safe-area overlay and large-format presets
- **v0.6**: system-font Unicode text to true editable compound paths, cubic contours with holes, SVG round-trip
- **v0.7**: labelled and explained toolbox, onboarding, sample job, line/hexagon, corner resizing, internal clipboard, correct style duplication, nudging, safe quarter-rotation/mirroring
- **v0.8**: unified dark/light theme, named objects and filtered list, persistent flat groups, atomic group actions, accurate property geometry, page centering and configurable grid

- **v0.9**: editable curve nodes on the canvas (anchor and control handle move, segment split, guarded deletion, undo) — Windows/Linux CI passed; manual production acceptance pending

- **v0.10 (I02)**: closed-shape boolean union, subtract, intersection, XOR with native editable compound paths, hole preservation, guarded operation failures (Windows/Linux CI passed; manual/production QA pending)

- **Tool-dock usability refresh**: narrow optional-expanded icon rail, figures as flyout, image and preflight shortcuts, context-sensitive text/node inspector, discoverability via hover and F1 (new CI acceptance pending)

## Long-term milestone plan

See **[the 20-iteration development plan](ROADMAP_20_ITERATIONS.md)** for the ordered backlog I01–I20, acceptance requirements and version targets.

## Delivered in v0.15 / v0.16 source
- **v0.15**: HP-GL/1 PLT export, curve flattening, cut/travel preflight, safe machine-coordinate checks and CLI; Windows/Linux CI passed. Physical plotter validation remains pending.
- **v0.16**: DXF R2000 polyline export for CAD/CAM hand-off, visible-layer preservation, mm/inch units, selectable Y origin, curve tolerance, offsets and CLI preflight/export; Windows/Linux CI pending. No direct laser/CNC control or G-code generation is claimed.

## Next milestones — highest priority
1. Real Windows 7/10 and Linux performance benchmarking (2/4/8GB RAM, 1k–100k objects)
2. Stronger typography: font choices, shaping, line height, kerning, Cyrillic layouts, text-to-path
3. True multi-page documents and reusable master elements; non-destructive clipping/masking
4. Professional print pipeline: color profiles, CMYK and PDF/X; real print-shop validation
5. Native editable imports for more SVG constructs and independent CDR compatibility analysis
6. Editable CDR export if feasible and legally/technically defensible; otherwise a documented interchange workflow

**CRITICAL**: having an import converter for CDR does not provide editable CDR compatibility. No CDR output, 100% fidelity, PDF/X or Windows 7 runtime support is claimed. Hardware/production validation has not yet been performed.

OpenCorel remains a provisional working name pending trademark clearance before distribution.
