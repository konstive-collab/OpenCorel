# v0.2 development preview (unreleased)

Source branch: `feat/graphics-workspace-v02`.

## Implemented in branch
- Familiar independent graphics workspace: menu, tool strip, contextual information, rulers, layers/objects dock, color swatches, page bar.
- Grid display and 10 mm snapping; keyboard shortcuts and navigation.
- Layer inspector, selection, edit controls, object order changes, history (50 snapshots).
- Native file chooser for OCD / supported SVG subset.
- Geometry validation improvements and safer temporary-file saves.
- RGB vector PDF proof export for single-page documents up to 5000 mm. **Not PDF/X or CMYK.**
- Closed shape tessellation/triangulation with basic caching.

## Deliberate omissions
- Not a CorelDRAW compatibility layer yet: no native CDR read/write, and no editable CDR round-trip.
- No AI/EPS/PSD/TIFF import, no true bitmap design workflow.
- No professional typography engine or color-management-certified print pipeline.
- No multi-page editing (page bar represents only first page).
- Windows 7 not validated and modern GUI dependencies may prevent it.

## Important caveat
Windows/Linux CI proves compilation and unit tests only, not accurate GUI rendering or professional print fidelity. Manual hardware acceptance is required before company rollout.
