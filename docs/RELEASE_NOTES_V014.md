# OpenCorel v0.14 Alpha — Typography iteration I06 (in progress)

## Professional path tools — latest iteration

**Persistent Bézier node modes:** the node inspector now stores Corner, Smooth
or Symmetric mode per interior cubic anchor. Smooth and Symmetric preserve
coupling while a handle is moved; Corner allows independent handles. A new
**Free** command unlinks the two handles without changing the curve shape.
Node modes are stored as optional metadata per object in the native `.kdr`
format; earlier documents have no such metadata and remain readable.

**Structural editing:** Reverse Direction reverses the control-point order and
preserves the cubic trace and mode assignments. Close and Open alter only the
closure flag; no nodes or handles are destroyed. These operations also apply
to eligible polylines and compound paths (for compound paths, all contours
change together).

**Topology safety:** when a node is inserted or removed, mode entries are
reindexed to follow the original anchor. Invalid/duplicate mode addresses
are rejected by the document validator. Commands on unavailable layers or
groups are denied. Metadata edits, contour reversal and closure each use
a single Undo checkpoint. A drag still checks validity before creating its
first checkpoint.

**Compatibility:** `schema_version=1` remains unchanged; the new
`node_modes` member is optional and skipped when empty. Legacy files remain
readable without migration; older builds may discard the newer metadata when
resaving, so keep backups when exchanging project versions.

**Regressions:** independent core tests verify KDR roundtrips, linked and
unlinked handles, smooth/symmetric lengths, metadata remapping, exact reversed
cubic geometry, no-op closure, degenerate inputs and invalid mode references.
Desktop tests cover persisted mode Undo/Redo and path operation Undo/Redo.

**Limitations:** automatic coupling currently applies only to internal nodes
with both adjoining cubic handles, not to the initial/end anchor of an open
path or to the implicit closing segment. There is no full-fledged persistent
node mode selection across multiple objects. Manual Windows mouse testing is
still required; cross-platform CI alone does not certify input feel.


## Implemented in PR #23
- Installed system font families can be selected in the existing editable text inspector.
- Search-as-you-type filter and virtualized rows improve usability for large font collections.
- Font-family enumeration is cached within the typography service, without copying or bundling font files.
- Missing font families are explicitly warned about; the saved family string is preserved rather than silently replaced.
- Editable text can be converted to uppercase/lowercase using Unicode-aware Rust string operations, including Cyrillic.
- Character and line counts are displayed for text objects.
- Invalid text-format feedback now reports typography validation errors rather than misleading color warnings.
- Regression tests for sorted unique fonts, cached result stability and installed-family availability.

## Next slice: non-destructive font-faithful canvas preview
- The canvas now attempts to outline up to 600 characters using the same system-font shaping/outlining engine used by explicit text-to-curves.
- Outline geometry is kept in a bounded 24-object cache; original text objects remain editable in .kdr.
- Shaping or font failures safely fall back to the existing lightweight text preview, rather than destroying or replacing the editable object.
- Cache entries are invalidated by changes to text geometry or typography metadata.
- New tests cover cache keys, large text and empty-text fallback.
- Known caveats: no rich text runs, interactive caret or guaranteed cross-platform visual parity; shape bounds and selection boxes still use approximate text metrics.

## Additional text editing tools
- The selected editable text object now offers literal find and replace in the inspector.
- Replacement is Unicode-safe, case-sensitive and bounded to 256 KiB output.
- The operation respects existing layer locks, document checkpoint/undo, and keeps text editable.
- New tests cover Cyrillic, emoji, special characters, empty search, no-match cases and size overflow.
- This is object-local find and replace, not a document-wide search or rich-text editor.

## Visual bounds upgrade
- For supported on-canvas text, selection frames use the cached glyph-outline control-hull bounds instead of the approximate text-width estimate.
- Cache signatures ensure geometry derived from an older position, font, or string is not reused after edits.
- Invalid (NaN/Infinity) glyph coordinates are rejected for selection bounds. Empty contours continue to use the legacy frame.
- New tests cover Bezier control extrema, invalid coordinates and stale-cache rejection.
- The bounds are conservative (Bezier control points can lie outside the actual curve). Mouse hit-testing and document-wide alignment still use the legacy document model, and are NOT yet fully glyph accurate.

## Mouse selection and viewport hit-testing
- Added `Document::hit_test_with_bounds`, a non-persistent viewport override for selection bounds. Existing `hit_test` delegates to it and remains API-compatible.
- Canvas clicking and dragging now use measured text glyph bounds when available, and fall back to legacy bounds otherwise.
- Selection checks still respect topmost object ordering, locked/hidden layers, and vector clip links.
- Picking is based on unsnapped mouse coordinates; tool placement and dragging may still use grid snapping.
- New core tests verify custom bounds, stacking order, layer restrictions and invalid queries.
- Picking still uses the glyph **bounding envelope**, not a per-pixel inside-glyph winding test. Cache may not be populated for out-of-viewport artwork; exact caret and individual character selection remain pending.

## Canvas-launched text editor
- Double-click an editable text object with the selection tool to open a multiline editing window near the pointer.
- Standard egui TextEdit provides text caret, keyboard input, keyboard/mouse selection and platform clipboard while the text is in the editor.
- Apply commits one document checkpoint so Ctrl+Z undoes the whole text editing session; Cancel, Escape or closing the window drops the draft.
- The editor rejects changes on locked/hidden layers, changes to deleted objects, oversized text and concurrent modifications to the original text value.
- The canvas stays visible but input is suspended while a text draft is active. Global shortcuts are also suspended to avoid accidentally changing the document while typing.
- Regression tests cover apply/undo/redo, cancel/no-op, layer-lock rejection and conflict rejection.
- Important limitation: the editable caret lives in a floating text window, **not yet** directly in glyph contours on the canvas. True in-place caret positioning and per-glyph selection remain upcoming work.

## Bezier responsiveness hotfix (post Windows alpha test)

User testing reported that the Bezier tool slowed down during curve drawing.
Source inspection showed two avoidable workloads:
- each added anchor made a full `Document` undo snapshot (including large
  embedded artwork); a growing curve could therefore repeatedly clone
  unrelated objects and bitmaps;
- every repaint re-sampled every cubic segment, even during a simple pan or
  mouse hover.

The working branch now:
- stores **one undo checkpoint per newly created curve** (from the first
  actual segment), not one per anchor; Undo during or after drawing removes
  the whole curve, and Redo restores it;
- caches adaptive cubic subdivision keyed to control points and zoom scale;
  keeps at most 32 cached paths and 20,000 sampled points per path;
- adds a live rubber-band preview between the last anchor and mouse pointer;
- checks the actual layer containing the draft, including after the active
  layer changes, so locked or hidden layers cannot be edited accidentally;
- tests 250-anchor construction, undo/redo, abandoned one-point drafts,
  layer-lock changes, cache invalidation, budget limits and reuse.

These changes address identified sources of lag. They are **not** a claim
that Windows interactivity has been benchmarked: latest-head CI and a user
re-test of the Windows artifact are required before considering the
regression closed. Drag-to-create smooth handles is a separate upcoming
Bezier feature; this build is still a click-based curve tool.

The product-level feature order now follows
[CorelDRAW-workflow parity first](CORELDRAW_PARITY_FIRST.md).

## Bezier mouse interaction — smooth handles and closed paths

Building on the responsiveness work, the Bezier tool now supports:
- **Click:** insert a cusp/corner anchor with straight fallback handles.
- **Press and drag:** insert an anchor at the pointer press origin and shape
  opposing symmetric incoming/outgoing cubic control handles with the drag.
- **Live preview:** a 16-step temporary cubic preview respects the outgoing
  handle rather than drawing a misleading straight rubber band; handle is
  shown in a contrasting color.
- **Close:** click the initial anchor after constructing at least two
  segments to set the existing Bezier object's `closed` flag.
- **Finish:** double-click or press Enter for an open curve.
- **History and safety:** all anchors remain one undoable path; tangent
  updates do not checkpoint each frame and respect locked/hidden layers;
  switching tools, Escape and undo/redo discard stale in-progress handles.
- **Regression tests:** symmetric geometry, invalid/nonfinite tangents,
  Unicode-safe unrelated text functionality, drag-to-shape, cusp after a
  smooth anchor, closure and layer-lock denial.

Known limitations:
- This is symmetric/smooth anchor creation, not a complete node-type
  system: independent/asymmetric handles, tangent snapping and curve
  continuation from previously saved paths still need dedicated work.
- GUI drag/drop flow is not hardware-certified by Rust unit tests. It requires
  explicit Windows acceptance after the newest workflow's CI finishes.
- Double-click completion does not create an extra point at the second click.
  Use Enter to finish with the current last anchor.

## Earlier node-editor milestone: one-shot tangent geometry (superseded by persistent modes above)

The node inspector now exposes three **one-shot** operations for an interior
Bézier/compound-path anchor with both incoming and outgoing cubic handles:
- **Corner:** collapse both handles onto the anchor.
- **Smooth:** align tangent directions, keeping their original lengths.
- **Symmetric:** align tangent directions and equalize handle lengths.

If both handles were previously collapsed by Corner, Smooth/Symmetric
reconstruct a usable tangent from the neighboring anchors using one-third
segment-length defaults. Completely coincident neighboring anchors are
rejected, leaving the shape untouched.

The operations are computed and bounds-validated before changing a shape.
Invalid addresses, terminal nodes, locked/grouped objects and non-finite handle
coordinates are rejected without mutating the document. No-op commands
do not add undo entries. Existing `.kdr` document schema is unchanged, but
these are geometric commands, not persistent node-type settings: handle
coupling after later drags is not yet implemented.

Node drag history was also optimized: grabbing a node no longer deep-copies the
whole document; the first valid movement records a checkpoint, and the rest of
the continuous drag shares it. No-op/invalid positions create no history
entries. Regression tests cover cubic and compound paths, different handle
lengths, invalid/degenerate cases, serialization, undo/redo and lock behavior.

Manual Windows checks: draw a three-anchor curve, press F10, select the middle
anchor, use all three commands with Ctrl+Z between operations. Drag the
control handles, and confirm Ctrl+Z after a continuous drag restores the
original path in one step.

## Verification
GitHub Actions Windows and Linux runs must complete against the latest PR head before merge. Early runs for the initial system-font picker passed, but do not imply later additions have passed.

## Still not implemented or not certified
- Exact WYSIWYG text shaping on the canvas: current canvas text preview is approximate.
- Rich-text spans, per-character formatting, caret-on-canvas editing and full font feature controls.
- Bidirectional text and all script combinations.
- Verified cross-application typography fidelity, embedded font files, CDR interchange and prepress-grade PDF export.
- Runtime font installation refresh (font list is cached for the editor session).
- Manual GUI acceptance on Windows and Linux.

## Manual smoke checks
1. Add a text object containing "Привет, OpenCorel!", select it, and search for an installed font.
2. Pick a font; save as .kdr, reopen, and verify that the selected family is preserved.
3. Search for an absent name and check the missing-font warning.
4. Apply upper/lowercase actions to Cyrillic text; verify undo and redo, and verify locked layers reject edits.
5. Reopen an old .kdr document and confirm typography metadata migration remains backward compatible.
