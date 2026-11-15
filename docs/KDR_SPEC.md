# Kodi Draw project — .kdr

The primary native project extension is `.kdr` (**Kodi Draw**). It is independent of CorelDRAW CDR and must not be renamed to .cdr.

Format v1 uses a ZIP container containing:

- `manifest.json`: `format`, `version`, `document`, `sha256`, `generator`
- `document.json`: editable page, objects, styles, curves, layer visibility and ordering (current OCD schema v1)
- `preview.svg` (optional): a self-contained preview where under 30 MiB; omitted for asset-heavy projects. The editable JSON is authoritative

- SHA-256 integrity check of uncompressed document.
- Compression via DEFLATE. Maximum archive size 128 MiB, uncompressed document 96 MiB.
- Validation on load and save; temporary file and atomic replacement where supported.
- Future changes require a new KDR container version or documented compatible extensions.
- Native objects now include embedded PNG/JPEG photographs, compressed with the project, so they do not depend on external file paths. WebP/GIF/BMP/TIFF normalize to PNG on import.
- This first format version does **not** embed fonts, separate ICC profiles, external links or original CDR source documents. Do not treat it as a professional archival or print-certified format yet.
- Raster artwork is stored as Base64 in document JSON. Undo history is memory-limited; the format may evolve to binary ZIP entries for better handling of many gigabyte-scale projects.
- Legacy `.ocd` (plain JSON) remains supported for import and explicit export.

Tests include round-trip, corrupt checksum rejection and preserving previous saved work after a rejected save.
