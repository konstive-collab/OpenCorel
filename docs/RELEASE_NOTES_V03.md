# v0.3 — File subsystem preview

## Added in development branch
- `.kdr` **Kodi Draw** native compressed project with versioned manifest, checksum, page and layer content, editable objects and SVG preview.
- Old `.ocd` project files still load; explicit legacy export remains possible.
- Native format-aware UI open/save, default Save As .kdr, and a command-line file converter.
- Editable SVG subset and SVGZ import.
- Complex external drawings can be imported as a **single SVG vector illustration** that moves and scales as one object. Rendered with resvg and texture caching.
- Experimental LibreOffice/Poppler adapters for CDR, CMX, AI, EPS, PS, ODG, DXF, WMF, EMF, WPG, Visio, PDF and other listed formats.
- KDR tests: round-trip, checksum mismatch and preservation of existing documents when saving invalid data.

## Do not promise
- CorelDRAW round-trip: **CDR export is not implemented**.
- 100% visual fidelity, exact fonts, colors or dimensional scale.
- Editing every path or text object inside converted CDR/PDF/AI.
- Multi-page import, CMYK output, PDF/X or Windows 7 runtime compatibility.

## Practical workflow
1. Use **File → Open** to load the source file.
2. Observe the status text for the mode (native editable / simplified editable / placed vector) and any warnings.
3. Save a working copy to `.kdr`, leaving the CDR/PDF/AI original untouched.
4. Export SVG or proof PDF when appropriate, then check scale and typography against the source.

Import may require separately installed LibreOffice or Poppler. See [format matrix](FORMAT_SUPPORT.md).
