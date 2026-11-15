# OpenCorel v0.11 Alpha — Non-destructive vector clipping (I03)

## Implemented
- Editable, persistent clip links between closed vector source shapes and raster/vector destination artwork.
- Commands in Masks menu and quick object toolbar; undo/redo for apply and detach.
- Selection hit-testing respects clip visibility and even-odd compound-path holes.
- Identical SVG clipping engine for on-canvas resvg proof, SVG export and downstream PDF/raster proof export.
- Bounded 4-million-pixel on-canvas preview cache; asset buffers are not hashed in full every repaint.
- Corrupted KDR links, locked/hidden layers, groups, self-links, cycles and invalid/open path sources are refused without changing the project.
- Backward-readable native `.kdr` schema version 1; clip links are optional in older documents.
- Test coverage for undo/redo, SVG/KDR, masked click geometry, preview pixels, PDF, invalid links and raster destinations.

## Limitations / pre-release acceptance
- Clipping sources must be **closed vector geometry**; raster alpha/luminance masks and nested masks are unsupported.
- Masked canvas is rasterized at up to 2048px per side / 4MP: at extreme zoom the preview can appear soft.
- SVG preview inputs beyond 42MB are explicitly rejected in the GUI; documents remain editable and may still export through supported paths.
- Imported SVG/AI/PDF round-trip clipping fidelity is not guaranteed.
- Manual GUI QA on representative devices, large raster assets, Windows 7, professional print proof and extended performance profiles are pending.
- No CDR export or PDF/X/CMYK certification.

## Build verification
- The automated two-platform CI for the implementation before the version metadata change passed: https://github.com/konstive-collab/OpenCorel/actions/runs/43759897308
- Version-bump revision requires a fresh CI run; do not claim its pass until it finishes.
