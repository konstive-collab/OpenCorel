# OpenCorel — roadmap and status

Goal: fast, legally independent vector graphics editor for advertising, posters and maps, especially on older Windows/Linux workstations.

## Delivered in source (automated CI; manual acceptance still pending)
- **v0.1**: millimetre-based document engine, desktop GUI/CLI, OCD, SVG export
- **v0.2**: Bézier paths, layers, grid/snap, rulers, palette, undo/redo, native file pickers, PDF RGB proof
- **v0.3**: KDR ZIP format with checksum, CDR/CMX/AI/PDF conversion adapters, partial SVG editability
- **v0.4**: embedded PNG/JPEG photos, WebP/GIF/BMP/TIFF import, cached image previews, PNG/JPEG RGB proofs, bounded undo history, multi-select, alignment and distribution
- **v0.5**: text formatting & multiline SVG, layout preflight checks, safe-area overlay and large-format presets

## Next milestones — highest priority
1. Real Windows 7/10 and Linux performance benchmarking (2/4/8GB RAM, 1k–100k objects)
2. Stronger typography: font choices, shaping, line height, kerning, Cyrillic layouts, text-to-path
3. True multi-page documents and reusable master elements; non-destructive clipping/masking
4. Professional print pipeline: color profiles, CMYK and PDF/X; real print-shop validation
5. Native editable imports for more SVG constructs and independent CDR compatibility analysis
6. Editable CDR export if feasible and legally/technically defensible; otherwise a documented interchange workflow

**CRITICAL**: having an import converter for CDR does not provide editable CDR compatibility. No CDR output, 100% fidelity, PDF/X or Windows 7 runtime support is claimed. Hardware/production validation has not yet been performed.

OpenCorel remains a provisional working name pending trademark clearance before distribution.
