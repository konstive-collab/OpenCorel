# OpenCorel v0.8 Alpha — Object organization and interface polish

## Implemented
- Cohesive graphite/dark visual system with teal highlights, compact controls, consistent right dock and light-theme alternative.
- Persistent KDR `Object.name` and `Object.group_id` fields with backward-compatible JSON defaults and validation.
- Flat groups (Ctrl+G / Ctrl+Shift+G): clicking one member selects the whole group; drag/nudge/colour/edit order applies to the whole selection.
- Independent groups when using Ctrl+D or internal Ctrl+C/V, remapping copied group IDs.
- Search/filter by custom object label, text content, shape category and numeric object ID.
- Geometry inspector: exact X/Y for selected groups and exact width/height for individual resizable figures.
- Adjustable snapping/grid/ruler step from 0.5 to 1000 millimetres; one-click centre-on-page.
- Atomic delete, recolour, position, nudge and z-order actions when any selected layer is locked or hidden.
- Tests for grouping, serialisation, compatibility with prior KDR data, duplication, safe transforms and geometry layout.

## Limitations
- Grouping is flat; nested groups and visual hierarchical group tree are not implemented yet.
- Multi-object resizing with group handles, arbitrary-angle rotation, Boolean path operations and on-canvas node editing remain incomplete.
- User-defined object names and group membership are KDR metadata, not fully preserved through SVG/PDF/CDR conversions.
- No professional CMYK/PDF/X, native editable CDR export, verified Windows 7 or press certification.
- Automated build/tests are not a substitute for manual GUI and large-format production validation.
