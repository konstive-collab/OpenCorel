# OpenCorel v0.19 — Drawing & Production Mega

## Delivered in source
- Freehand pencil; 30,000-point bound, tolerance-based simplification,
  standard editable polyline generation, preview and one-step Undo.
- Two-direction marquee selection; group-aware, layer-aware,
  additive selection with Shift.
- Eyedropper/style clipboard with fill, outline, gradient, opacity,
  dash/cap/join and text font properties; atomic clipboard application.
- Native grouped dimension annotations (width/height millimetres).
- Selection-focused viewport zoom.
- Non-destructive paper-enlarged trim-mark SVG and RGB PDF proof
  (8 marks), desktop export panel and CLI commands.
- Persisted native KDR geometry; automated Win/Linux tests for
  core interaction and output.

## Limitations
- Freehand input is mouse-based; no tablet pressure or tilt support.
- Trim marks are **not** automatic bleed extension, nor PDF/X, CMYK,
  ICC soft-proofing, registration/control bars or ISO print certification.
- Original CDR fidelity, native CDR export, master pages and complete
  CorelDRAW typography remain unsupported.
- Hardware runtime validation and Windows 7 testing are outstanding.

## QA
Cargo workspace tests, release tests, clippy, Windows/Linux desktop and
CLI builds, document/batch export regression checks and independent
DXF/PDF readers in GitHub Actions. Manual UX and real print trials pending.
