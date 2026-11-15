# OpenCorel v0.9 Alpha — Interactive Node Editing (I01 of I20)

## Delivered in source
- Dedicated **Node tool** (F10) with visible, clickable anchors and cubic Bézier control handles on the canvas.
- Works with polyline, Bézier and compound path types. Supports drag-to-move anchor/handle, Delete-to-remove anchor, arrow-key nudging and snap-to-grid during dragging.
- Double-click near a segment creates a new node. Bézier segments split using the de Casteljau algorithm, preserving exact curve geometry.
- Explicit refusal for locked layers, grouped objects and unsupported shapes; no silent conversion of text or raster artwork.
- Undo/redo snapshots for editing actions; operations round-trip in native KDR because they modify the existing geometry model.
- Unit tests for exact curve split, handle motion, polyline insert/delete guards, compound path persistence, invalid coordinates, nearest segment and editor node undo.
- Added [20-iteration roadmap](ROADMAP_20_ITERATIONS.md) with acceptance gates and future priorities.

## Important limitations
- No fully fledged sculpting/smooth node handles, automatic tangent constraints or interactive boolean operations yet.
- On very complex compound paths only the first 750 handles are displayed and 4000 segments are searched for insertion to protect weak PCs.
- Insertion finds an approximation of the pointer's closest parameter and then performs mathematically exact curve subdivision at that parameter.
- Manual QA on real company artwork and on Windows 7 has not been performed; CorelDRAW-compatible editing fidelity is not certified.
- No CDR export, CMYK/PDF/X or multi-page output yet.
