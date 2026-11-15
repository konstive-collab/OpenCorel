# OpenCorel v0.10 Alpha — Boolean Vector Operations (I02/20)

## New

- Dedicated top-level Contours menu and selection-aware quick toolbar: **Union**, **Subtract A-B**, **Intersect**, **XOR**.
- Closed filled rectangular, elliptical, polyline, cubic Bézier and compound vector inputs.
- Numeric topology engine from geo 0.33.1, with contour simplification/flattening to straight cubic segments. Target geometric tolerance 0.1mm; bounded depth, vertices, complexity and coordinate range.
- Even-odd compound path reconstruction and output that preserves holes and disconnected regions as editable native KDR geometry; style inherited from first selected object.
- Atomic editor workflow with Ctrl+Z undo, safe handling of locked layers and groups. No mutations when input invalid or result empty.
- Deterministic geometry area tests across all four operations; tests for containment hole, two islands, non-overlap, self-difference, editable KDR roundtrip, invalid/open text paths, 128 randomized rectangle overlap invariants and GUI undo/lock tests.
- v0.9 hardening audit and expanded debug/release/Clippy checks are prerequisites.

## Known limitations

- Two selected shapes only; no true compound multi-object Boolean reduction yet.
- Existing parametric Bézier curves and ellipses are approximated by straight segments, so the exact analytic shape/handle layout is not retained.
- No automatic smoothing after operations. Multi-contour input uses even-odd fill; self-intersecting geometry is not guaranteed.
- Open paths, strokes-only artwork, text (before converting to outlines), groups and raster/placed vector images are refused.
- Complexity limits: input point cap 20,000, output vertex cap 25,000, 128 contours, coordinate range ±200,000 mm. If exceeded, the document is left unchanged.
- Results have not undergone visual GUI acceptance on production hardware; this is not CDR export, CMYK management or PDF/X certification.

## QA

The release is gated on Windows and Linux debug/release tests, static analysis and artifact publication. Manual GUI interaction, real CDR and press output tests remain required.
