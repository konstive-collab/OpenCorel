# OpenCorel v0.4 — Raster artwork and layout tools

This is a source release preview, not a certified CorelDRAW replacement.

## New
- Import and place photos/logos as native embedded image objects: PNG, JPEG, WebP, GIF, BMP, TIFF (the latter four normalized to PNG)
- Store bitmap assets in .kdr without requiring the original file path
- Drag-and-drop image placement, position/scale in millimetres, aspect ratio lock and effective DPI display
- Faster movement of large images without cloning their encoded bytes per frame
- Memory-bounded undo history (96 MiB target) and capped preview texture caches
- Save KDR even if its optional SVG preview would be too large
- Export full page as PNG/JPEG RGB proof at selected DPI, with page-pixel limits protecting old computers
- Shift multi-select, Ctrl+A, align edges, distribute centers, move/duplicate/delete/edit multiple items
- Tests of raster import, KDR image round-trip, bitmap proofs, geometry alignment, and overflow/error paths
- Release workflow publishes both desktop and command-line executables

## Known limitations
- 96 DPI used as default interpretation for imported image dimensions; original EXIF orientation and ICC profiles are not guaranteed
- Only first frame/page imported from animated GIF or multipage TIFF
- PNG/JPEG proof files are RGB (no CMYK/PDF/X; not color-certified)
- The max image import is 40 megapixels, 16,000px per side, 40MiB compressed
- Bitmap proof export capped at 24 megapixels and 10,000px per side
- CDR import remains experimental, non-lossless and dependent on LibreOffice/Poppler; editable CDR export not implemented
- Windows 7 is not validated; source/build CI is not a GUI manual test

Manual end-user validation will be scheduled when the editor reaches a fuller functional milestone.
