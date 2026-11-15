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
