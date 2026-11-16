# OpenCorel v0.14 Alpha — Typography iteration I06 (in progress)

## Quality stabilization — regression and predictable editing

- **Hidden-guide snap bug fixed:** turning guide visibility off also disables
  invisible guide capture, even if guide snapping remains enabled.
- **Correct numeric Undo:** continuous DragValue editing of one guide
  coordinate records only one checkpoint. Validation/no-op input records none;
  other commands and new editing sessions remain independent.
- **Offset safety gate:** closed paths now reject oversized inward offsets
  that flip local edge direction (e.g. a 10×10mm square inset by 6mm).
  Unsafe reversed/collapsed edges, self-intersections and degenerate winding
  are refused, with the original object untouched and no Undo checkpoint.
  Self-intersections are not automatically repaired.
- **Acceptance note:** successful compiler/CI checks are not evidence that
  GUI ergonomics or CorelDRAW production workflows are mature. Manual
  Windows acceptance of object manipulation, ruler drags and export fidelity
  remains a release gate.

## Persistent guides and editable parallel contours

- **Persistent nonprinting guides:** horizontal/vertical guide positions are
  added to the optional `.kdr` document metadata. Old projects remain
  compatible, and saved positions survive reopen and page resizing.
  Guide values are validated (finite, within ±100,000 mm, max 256).
- **Interactive rulers:** draw a new vertical guide from the top ruler or
  a horizontal one from the left ruler. **Alt+drag** an existing guide
  to move it. Each completed gesture adds exactly one Ctrl+Z checkpoint;
  numerical editing, single removal and clearing all guides are also undoable.
- **Guide snapping:** independent X and Y snapping including guide
  intersections with zoom-consistent pixel tolerance. Visible-object anchors
  retain priority; grid snap is fallback for any unsnapped axis.
  Guide rendering is canvas-only, never included in vector/raster exports.
- **Editable signed contour offsets:** create a new parallel native polyline
  from a rectangle, ellipse, polyline or Bézier, retaining the original
  object as well as its style/layer. Offsets range from -1000 to +1000 mm
  (excluding zero) with sign defined relative to path direction. The result
  is independently editable; original geometry is not mutated.
- **Robust bounds:** 4096-point cap, degenerate/reversed segment checks,
  clamp for long miters on acute turns, finite ±100,000 mm coordinates,
  no unsupported clipped/grouped/locked edits. For curves, a finite sample
  approximation is used. Invalid collapsed/reversed edges and intersecting
  offset results are rejected rather than published silently.
- **Tests:** guide persistence/legacy compatibility, intersection snapping,
  invalid values, create/move/delete + undo, ruler drag, signed offset of
  lines/L-shapes/squares/rectangles/cubic curves, rejected degenerate
  shapes and copy-with-undo checks.

**Limitations:** guide drags are pointer based rather than a full selection
tool. Guide metadata is only used in KDR editor documents, not exported.
Bezier and ellipse parallel offsets are sampled polylines, NOT exact
geometric offsets or fully featured CorelDRAW contour effects. Colors and
appearance are copied, but source clipping/group references are not copied.
Manual Windows acceptance remains necessary.

## Freehand pen and geometric fitting — next professional toolset

A new **Freehand (H)** tool collects pointer samples during a continuous
left-button drag. On mouse release it creates **one native editable Bézier**
object (not a bitmap) with one Undo entry. Preview is a lightweight line
rather than re-fitting the entire sample set on every frame.

- **Adjustable drawing precision:** 0.5–12 screen pixels converted to
  millimetres using the active canvas scale. A bounded iterative
  Ramer–Douglas–Peucker pass removes redundant points.
- **Adjustable smoothness:** 0–1 tension-based cubic handle interpolation.
  Low values preserve angular details; high values create flowing
  interpolated cubic sections.
- **Closed smooth strokes:** optional checkbox creates an editable closed
  cubic contour with cyclic tangent continuity at the start/end seam.
- **Existing-path conversion:** F10 properties can turn an open or closed
  native polyline into a fitted Bézier without losing style or layer.
- **Simplify existing cubic:** resample up to 500 native cubic segments,
  reduce them within a user-entered millimetre tolerance and fit new
  segments; refuses when the operation would not reduce the count.
  Since it changes node topology it intentionally clears old per-node
  tangent-mode assignments; Ctrl+Z restores them exactly.
- **Safety and responsiveness:** rejects nonfinite/out-of-range geometry,
  tap-only gestures, locked layers, oversized strokes (4096 samples) and
  fitted paths with more than 2048 segments. An excessive gesture is
  discarded with a warning rather than silently truncating its geometry.
  Geometry fitting happens only on release or explicitly requested refit,
  never for every pointer frame.
- **Automated regression coverage:** collinear simplification, corner
  retention, smoothness, closed tangent seam, document validation,
  4096-sample budget, invalid values, undo/redo, locked layers and
  conversion/refitting of existing open and closed paths.

**Scope and caveats:** mouse/tablet pointer position is sampled, but pen
pressure/tilt is not yet captured. This is stroke-outline editing rather
than variable-width brush painting. Paths are approximate and must not
be treated as dimensionally exact technical vectors without review.
Manual Windows drawing tests remain required even if CI is green.

## Precision drawing — node snapping and smooth seam joining

- **Adjustable join distance:** the F10 panel now exposes an editable 0–1000 mm
  snap tolerance (default 2 mm), replacing the old hard-coded 2 mm button.
- **Smooth Snap Join:** in addition to ordinary snap join and bridge, a new
  command snaps the nearest ends, aligns both cubic tangents while retaining
  handle lengths, and stores a persistent Smooth node mode on the seam.
  The result remains native and fully editable, with live tangent coupling.
- **Visible node snapping:** View menu toggle (enabled by default) controls
  snapping for Bézier anchor creation and F10 anchor movement. Pixel tolerance
  can be adjusted between 2 and 30 px (default 10), so a change of zoom does
  not change the perceived snap radius. A small orange diamond indicates
  the active snap target.
- **Target priority:** visible cubic/compound/polyline anchors and rectangle
  corners; never hidden layers, bitmap pixels or imported SVG internals.
  Locked visible objects can serve as immutable reference targets.
  The edited object's own nodes are excluded; grid snapping remains fallback.
- **Bounded work:** snapping scans no more than 30,000 target points from
  8,000 objects per lookup, preventing unbounded cursor stalls on enormous
  imported outlines. Snap may not inspect later candidates past the limit.
- **Tests:** nearest-anchor selection, zoom-invariant tolerance, grid fallback,
  hidden/locked layers, invalid coordinates, large 50k-point path budget,
  smooth seam continuity and persistent node mode, join-tolerance rejection,
  and undoability.

Limits: no snapping to arbitrary curve interiors, intersections or guides yet.
Smooth joining is a single tangent alignment, not a global curve optimization;
extreme or degenerate geometry is rejected rather than silently flattened.

## Path connectivity — join / bridge / split (new)

Two separate **open native Bézier** paths can now be combined into one path
without flattening:
- Nearest-endpoint **Snap Join** (up to 2 mm tolerance): automatically
  chooses path direction, aligns the closest endpoints and shifts the
  second path's first control point with its starting anchor.
- **Bridge Join**: automatically selects nearest endpoints, preserves
  original geometry and inserts one linear cubic between them.
- Existing interior-node tangent modes from both paths are preserved and
  their indices are remapped into the combined document object.
- **Split at Anchor**: an open cubic can be split at a selected interior
  endpoint into two editable objects. Styles and object appearances are
  copied; tangent modes are partitioned and renumbered correctly.
- **Continue Existing Curve**: F10 on a selected editable open Bézier
  exposes an action to resume the Bézier pen on the same object. New segments
  append directly to that path. Selecting the saved last anchor for a drag
  does not add a zero-length segment; it shapes the outgoing tangent.
  History is recorded lazily: entering/exiting without edits creates no
  checkpoint, and all additional segments or tangent edits are one Undo step.
- Undo restores the original object graph in **one** step. Redo restores
  the joined or split result.

**Fail safely:** operations reject locked/hidden layers, grouped objects,
clipping links, mismatched style/appearance/layer, closed/unsupported paths,
invalid anchors, out-of-range snapping tolerance, unsafe coordinates and
large post-join path sizes. They do not silently delete clipping links or
lose individual path effects.

**Limits:** snap tolerance is currently fixed to 2 mm; split only applies
to open cubic paths and a previously existing interior anchor; native
polyline and compound-path connection is not yet included. Bridge join
adds one straight control segment and does not produce automatic smoothness
across that seam. The first selected object retains identity/style/name,
while the second object's independent label is discarded on joining.
Manual Windows GUI acceptance remains required.

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
