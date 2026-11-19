# OpenCorel v0.20 — Professional Appearance, Typography & Vector

Implemented:
- 7 gradient presets, 2–32 stops, RGB interpolation, selected-style import, GUI color-strip editor, SVG/PDF/KDR persistence.
- Unicode-aware case-sensitive/insensitive Find/Replace on one or all pages, per-object result navigation, case conversion and Undo.
- Convert rectangles, ellipses and polylines to editable cubic Bézier segments; simplify open paths; stitch compatible open endpoints with preflight validation.
- Publication asset inventory: used fonts, RGB colors, text, raster/vector images, groups, clipping links and gradients.
- New CLI: inventory, find-text, replace-text, gradient-preset. Extended independent export smoke.
- Regression tests: gradients, Unicode Cyrillic, page states, locked layers, path geometry, undo, inventory.

Limitations: no native editable CDR import/export, complete OpenType shaping,
ICC/CMYK/PDF-X, connected printer/tablet certification or verified Windows 7 support.
RGB vector/PDF workflows must undergo manual review with real-world designs.
