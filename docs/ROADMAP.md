# OpenCorel — roadmap and status

Goal: fast, legally independent vector graphics editor for advertising, posters and maps, especially on older Windows/Linux workstations.

## v0.24 — Raster Studio

- Retained original PNG/JPEG, bounded edit settings and derived PNG in KDR.
- Crop, rotate, mirror, resample, RGB tonal correction, effects, transparency and chroma-key removal.
- Before/after thumbnails, explicit apply, reset, original/result extraction, atomic selected-image batch and CLI JSON automation.
- Original source bytes included in document/history, symbol, array, clipboard and page memory estimates.
- Existing SVG/PDF/PNG/JPEG renderers consume the derived payload; tests check raster pixels and PDF image objects.
- Limits: 24 MP source for editing, 40 MiB per encoded payload, 80 MiB estimated document growth budget, 32 objects per batch. Existing 40 MP import remains available. EXIF normalization, brush/mask editing, AI segmentation and ICC/CMYK remain future work.
- Automated CI and manual UI acceptance pending.

## v0.23 — Linked symbols and local component libraries

- Document-local symbol definitions and linked instances stored as additive, serde-default KDR metadata; schema version remains unchanged.
- Create a symbol from a visible, unlocked, ungrouped, unclipped selection on one source layer; insert grouped instances on the active layer.
- Update a definition from a linked source group and propagate to all instances, mapping geometry into each instance's prior frame and preserving placement. Text metrics may produce small residual frame differences.
- Detach a linked instance without flattening or deleting its editable objects.
- Portable JSON OCLIB library with atomic writes, import/export, search and category filters.
- Link-preserving copy/paste, duplicate, array repeat, page duplication and cross-page transfer.
- Validation budgets: 500 objects / 8 MiB per symbol, 512 symbols / 64 MiB per OCLIB file, 32 MiB of embedded symbol definitions per KDR document.
- SVG/PDF export continues to emit ordinary editable geometry. Nested symbols and per-instance overrides are not supported. Node editing is available for individual objects inside a linked instance; update the definition afterward.
- Automated Windows/Linux CI passed: workspace tests, release tests, Clippy, desktop/CLI builds, export smoke tests and independent DXF parsing ([run #45471502966](https://github.com/konstive-collab/OpenCorel/actions/runs/45471502966)). Manual UI acceptance, crash/restart simulations and large-document performance QA remain pending.

## v0.22 — Reliability and crash recovery

- Atomic KDR recovery snapshots on a 45-second cadence while dirty, with a 2-second edit debounce.
- Startup discovery of the newest valid recovery snapshot; explicit restore/discard choice.
- Save / discard / cancel guard before replacing a dirty document or closing the application.
- Crash reports contain version, timestamp, panic location and message, but never artwork contents.
- Unit tests for recovery round-trips, failed-save preservation, dirty state and guarded document replacement.
- Still requires manual crash/restart testing on Windows and Linux; autosave is not a substitute for saving the project deliberately.

## v0.21 — QR, Palette Management and Cross-page Object Explorer

- Real editable QR Module 2 path geometry, four error correction levels, protected quiet zone, print-sized validation.
- Atomic all-page RGB replacement for fills/strokes/gradient stops, read-only impact preview, locked-layer guards.
- Indexed object search by type, name, text, layer and color with result navigation.
- CLI commands, Win/Linux smoke and KDR/SVG/PDF round-trip regression.
- Not equivalent to production-wide CDR/CMYK/PDF/X or certified QR print scanning.

 Professional Appearance, Typography & Vector

Source includes 7 multi-stop RGB gradients, Unicode find/replace across 64 pages,
case transformations, cubic primitive conversion, open-path simplification/join,
on-demand font/color/image inventory, additional CLI automation and regression
coverage. Windows/Linux CI and manual visual acceptance gates remain mandatory.
CDR fidelity, CMYK/ICC/PDF-X and Windows 7 runtime verification are outstanding.

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

## v0.18 Studio / Production (source implemented, automated QA)
- Per-page guides, ruler creation, exact editing, smart snapping and nonprinting overlays.
- Complete layer rename/stack/reorder/empty deletion; safe object layer and page transfers.
- Direct vector glyphs for essential tools; Ctrl+K searchable command palette; page/z-order keyboard navigation.
- True combined multipage PDF (RGB proof), independent pypdf verification on Win/Linux; CLI export-pdf-book.
- Whole-publication page-attributed preflight, KDR persistence and undo/redo.
- Production acceptance still requires manual review against real CDR projects and actual print equipment.

## v0.17 — Professional Workspace (CI passed)
- Multi-page KDR up to 64 independent pages with object/layer/style/clipping preservation, page switching, duplication and deletion.
- Page tabs, page inspector, professional batch export dock, SVG/PDF RGB/PNG/JPEG per-page staging.
- CLI page introspection/creation/duplication and automated whole-document export.
- Remaining: master pages, true combined multipage PDF, full CDR/CMYK, nested groups, 100k-object performance tests.

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
