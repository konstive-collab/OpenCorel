# OpenCorel v0.14 Alpha — Typography iteration I06 (in progress)

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
