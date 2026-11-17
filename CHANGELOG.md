# Changelog

## v0.19 STUDIO — alpha

- Added persistent object-level visibility and lock flags. Legacy KDR objects without these fields load as visible and unlocked.
- Rebuilt the Object Studio manager with searchable grouped hierarchy, collapsible groups, per-object visibility/lock controls, layer management, same-type selection and batch actions.
- Added atomic object duplication that preserves text formatting, appearance, group membership and complete internal clipping relationships; partial groups and clipping pairs are rejected.
- Added stable z-order commands: raise/lower one step, send to front and send to back while preserving relative selection order.
- Added layer rename/reorder, move selected objects to active layer, and delete-layer-with-transfer operations.
- Updated rendering, export, picking, snapping, node editing, typography, color, appearance and batch transforms to respect object visibility/lock state.
- Added regression tests for legacy defaults, object states, hit testing, grouping, duplication, clipping, z-order, layer transfer, validation and undo.

**Known limits:** groups are not nested; layer list order does not yet define artwork z-order; no isolation mode or dock-layout persistence. Alpha status and Windows GUI verification remain required.

## v0.18 alpha

- Added 93 native smart shape presets, text-on-path outlines and a four-page editable shape catalogue.
