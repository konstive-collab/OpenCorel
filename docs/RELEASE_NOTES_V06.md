# OpenCorel v0.6 — Text-to-curves

- Explicit conversion of text objects to real cubic Bézier contour geometry via installed fonts, Unicode shaping and glyph outlines.
- Even-odd fill for compound glyph holes; Lyon-rendered preview, editable contour controls.
- Native KDR round-trip and re-import of supported SVG filled even-odd compound paths.
- GUI conversion of selected texts (all-or-nothing); Ctrl+Z restores text. CLI: `opencorel-cli outline-text source.kdr outlines.kdr`.
- Tests cover compound holes, serialisation, SVG import, glyph paths, and conversion limits.

**Limitations:** font files must be installed at conversion time. Fonts are not bundled. Outlined text loses text semantics. Full typeface rendering on the live canvas, advanced contour editing on-canvas, commercial-font licensing verification, CDR export, CMYK/PDF-X and Windows 7 support are not complete. This is not a press-certified release.
