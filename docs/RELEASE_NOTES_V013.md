# OpenCorel v0.13 Alpha — Object appearance (I05)

## New in v0.13
- Persistent, per-object opacity and gradient fills, with linear and radial gradient definitions in SVG.
- Editable two-stop linear/radial gradients with angle controls for linear gradients.
- SVG stroke dash arrays and butt/round/square caps, miter/round/bevel joins.
- Object Appearance menu with numeric controls; a two-object style eyedropper copies the first selected object's style and extended appearance onto the second.
- Non-destructive KDR v1-compatible appearance metadata keyed by object ID; legacy native projects open without migration.
- Cached SVG-based canvas proof, covering gradient fills and opacity, with up to 4MP raster preview.
- Batch application, style reset, Ctrl+Z undo, blocked-layer checks, invalid colors/stops/opacity rejection, safe style copying and cleanup on deletion.

## Automated QA
- Rust core tests for KDR roundtrip, old schema, malformed and dangling appearance data.
- SVG markup validation for gradient stops, dashes, caps, joins and object opacity.
- PNG raster proof checks for expected red/blue gradient transition.
- GUI-level undo and eyedropper tests.
- Initial Windows + Linux CI success on feature revision d3d5d15: https://github.com/konstive-collab/OpenCorel/actions/runs/43762455146

## Known alpha limitations
- Only two gradient stops exposed in GUI (the serialized gradient model supports up to 32); color stops have sRGB hex values only.
- This is not ICC/CMYK-managed output; no PDF/X certificate or spot-color separation.
- Canvas gradient proof is resolution-capped, and large embedded art may exceed preview limits.
- Object eyedropper samples the underlying vector style of the first selected object, not a screen pixel.
- Manual visual color comparison, complex external SVG interoperability, CDR, professional print checks and extended performance testing remain pending.
