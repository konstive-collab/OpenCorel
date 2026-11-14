# OpenCorel roadmap

Goal: fast vector design on older computers for posters, large advertising sheets and maps, with a workflow familiar to CorelDRAW users.

## 0.1 — foundation
- Independent Rust document engine using millimetres
- Basic rectangle, ellipse and text objects
- Simple GUI, CLI, own OCD save/load and SVG export
- CI on Linux and Windows
- First compilation and hardware acceptance testing still pending

## 0.2 — useful editing
- Bezier paths, edit nodes, lines, snapping and guides
- Layer management, groups, zoom/pan
- Undo/redo, autosave and atomic writes
- Spatial indexing and clipping for heavy maps

## 0.3 — print and portability
- Font shaping, kerning, text-to-curves
- ICC/CMYK, bleeding, large-format PDF/X and prepress validation
- SVG import and image embedding
- Windows 7 dedicated legacy builds and tests

## 0.4 — compatibility research
- Legal CDR import research (libcdr etc.), version-specific test corpus
- Determine feasibility of editable CDR export, not guaranteed
- AI/EPS/CMX/PDF test fixtures and round-trip comparisons

The internal name OpenCorel needs a trademark review before distribution.
