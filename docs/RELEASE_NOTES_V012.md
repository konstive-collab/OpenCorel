# OpenCorel v0.12 Alpha — I04 transformation progress

## Implemented in this iteration
- Arbitrary-angle rotation of vector outlines around either the selection center or a numeric custom pivot, in mm.
- Skew along the X and Y axes using numeric angles, with safe angle limits.
- Rectangle rotation/skew converts editable geometry to a closed polyline; ellipse converts to cubic Bézier outline, preserving visual geometry rather than falsely retaining an axis-aligned parametric ellipse.
- 90-degree rotations retain existing supported primitives.
- Shared-pivot positive X/Y scale across selected objects and full groups, including placed SVG and raster artwork without decoding/replacing original asset bytes.
- Uniform text scaling updates editable font size; anisotropic text scaling is explicitly rejected pending text-shaping work.
- Numeric controls in the Object → precise rotate/skew menu.
- Reject non-finite coordinates, excessive angles, unsafe output bounds and partial-group selections atomically.
- Existing undo/redo history supports all successful operations.

## Open requirements
- Interactive pivot handles and free-transform canvas handles.
- Full fidelity of transformed text, raster artwork and complex imported SVG under arbitrary rotation/skew.
- Combine/break-apart beyond existing flat grouping and Boolean operations.
- Comprehensive manual GUI interaction and visual golden fixture review on target PCs.
- The feature is alpha, not a CDR or prepress compatibility claim.

## Test evidence
CI is pending for the current revision. Tests cover pivot rotation, inverse roundtrip, skew, multi-object scaling, group completeness, undo and no-mutation failure behavior.
