# OpenCorel v0.11 I03 — Clipping foundation (in progress)

This is a partial engineering milestone, **not a completed release**.

## Implemented

- Document-level persistent `ClipLink` relations, backward-compatible with existing native KDR v1 files because the field defaults to an empty list.
- `set_clip(source_id, target_id)` and `clear_clip(target_id)`, preserving both original editable shapes.
- Only closed vector geometry can be a clipping source; raster and embedded-vector art can be targets.
- SVG uses user-space `clipPath` and individual target wrappers; existing PDF exporter consumes this SVG.
- Reject missing objects, self-clip, nested/cyclic links, duplicate targets, grouped and locked/hidden edit operands.
- Remove automatically cleans dangling clipping references; validation rejects corrupted references when importing KDR.
- Regression tests cover KDR/JSON preservation, legacy documents, SVG markup, invalid edits and raster targets.

## Outstanding for full I03 acceptance

- GUI commands for create/detach clip and true on-canvas mask rendering, including selection/hit-testing.
- Automated SVG/PDF raster reference comparisons on Windows/Linux; visual GUI acceptance.
- Real SVG import/export parity tests and support for alpha/luminance raster masks.
- Handle visibility/reordering and clipping composition with groups and nested scene elements.
- End-to-end undo/redo of the new GUI commands and multi-document stress tests.

## Release rule

Do not mark I03 complete or merge until compilation, automated tests and Windows/Linux CI pass. No claim of full SVG mask fidelity or prepress conformance.
