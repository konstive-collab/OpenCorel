# OpenCorel v0.5 — Typography and layout preflight

## Implemented
- Per-text-object font family, weight 100–900, italic, paragraph alignment, tracking in mm and line-height multiplier
- Multiline text preserved in `.kdr`; SVG exporter emits text/tspan with editable font attributes
- Strict SVG importer reads simple styled text/tspan without silently dropping unknown mixed structures; complex cases use placed vector fallback
- Improved text box geometry for multiline and paragraph alignment, used in selection and hit testing
- Preflight inspection: off-page items, small/empty text, low effective bitmap DPI, dependency on installed fonts, RGB/PDF output constraints
- Preflight in GUI (File → Check Layout) and CLI (`opencorel-cli preflight poster.kdr`)
- Configurable DPI warning threshold, screen-only safe-area margin and A0/A2/large-format page presets
- Backward-compatible KDR and OCD object decoding: old documents get default text settings
- Regression tests for Cyrillic multiline text, SVG round-trip, corrupted typography settings and preflight conditions

## Current limitations
- Canvas text preview uses simplified proportional/monospaced fonts and does not fully reproduce tracking, kerning, italic and typeface; verify visual output in exported SVG/PDF and on the target computer.
- Text layout relies on approximate metrics; full HarfBuzz-style shaping, glyph outlines and font embedding are future work.
- No CMYK/ICC, PDF/X, editable CDR export, verified Windows 7 builds or print-shop quality certification.
- Safe-area overlay is only a guide; it is not an actual bleed generator or crop mark exporter.
- The preflight checker is a risk report, not a pass certificate.
