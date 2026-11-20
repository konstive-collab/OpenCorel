# OpenCorel v0.21 — QR & Production Workflow Expansion

## Shipping source
1. Offline, standards-based QR generator using qrcodegen 1.8.0, with live native vector preview. Each QR comprises grouped background rectangle plus editable CompoundPath black modules. Automatic four-module quiet zone; contrast and module size checks; up to 1024 UTF-8 bytes.
2. Document-wide RGB palette manager with fill, stroke and gradient-stop channel filters; atomic preflight of hidden/locked page layers; dry-run counting and Undo; select matching objects.
3. Multi-page Object Explorer with native search across objects (names, text content, kinds, layers and RGB colors), page navigation and selection.
4. CLI: qr-code, recolor-rgb, find-objects, KDR/save and SVG output smoke.
5. Core and editor regression tests for QR generation, serialization, locked layer protection, color replacement across pages, object search, Undo and export.

## Honest limits
- Model 2 QR codes need physical scanner/camera validation on print media; no certification claimed.
- No complete native CDR edit/save, ICC/CMYK, PDF/X, advanced production spot colors, or Win7 runtime certification.
- Large QR codes with thousands of vector contours may create heavy files; a complexity limit is enforced.
