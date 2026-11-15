# OpenCorel v0.7 Alpha — Workspace usability iteration

This release focuses on **discoverability and everyday editing** rather than adding another file format.

### Delivered
- First-run interactive onboarding, keyboard help (F1), sample project with layers/shapes/text, and starter document presets.
- Wider resizable left toolbox with visible tool names, live step-by-step contextual instructions and text-entry panel.
- New line and regular-hexagon creation tools (click or drag) alongside existing shapes, text and Bézier curves.
- Single-object resize handles on canvas; Shift preserves proportions. Editable vector curves can be scaled.
- Internal clipboard Ctrl+C/X/V for selected document objects; Ctrl+D duplicates without losing font settings.
- Geometric object rotation ±90° and horizontal/vertical reflection where safely representable; unsupported transforms fail explicitly.
- Arrow-key precise movement (1 mm, 10 mm with Shift).
- Simpler main toolbar and task-organized menus; honest page indicator instead of fake multipage controls.
- Russian task-oriented guide at `docs/USER_GUIDE_RU.md`.
- Explicit save-as for unsaved and imported documents; export file dialogs avoid silent overwrites and unknown output directories.
- Unit tests for transformations, style-preserving duplication, sample validity, clipboard size limit, resizing and undo.

### Known limitations
- Alpha: not equivalent to CorelDRAW, not production-validated.
- Internal clipboard is not the Windows/Linux system clipboard.
- Rotation and mirroring unavailable for live text and imported embedded images; convert text to outlines first.
- Text scaling is through font size, not resize handles.
- No CMYK/PDF-X, editable CDR writer, true multipage or validated Windows 7 runtime.
- Source formats that require external converters still depend on LibreOffice/Poppler.
