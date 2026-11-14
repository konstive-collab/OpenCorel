# Kodi Draw project — .kdr

The primary native project extension is `.kdr` (**Kodi Draw**). It is independent of CorelDRAW CDR and must not be renamed to .cdr.

Format v1 uses a ZIP container containing:

- `manifest.json`: `format`, `version`, `document`, `sha256`, `generator`
- `document.json`: editable page, objects, styles, curves, layer visibility and ordering (current OCD schema v1)
- `preview.svg`: thumbnail/preview for tooling; the editable JSON is authoritative

- SHA-256 integrity check of uncompressed document.
- Compression via DEFLATE. Maximum archive size 128 MiB, uncompressed document 96 MiB.
- Validation on load and save; temporary file and atomic replacement where supported.
- Future changes require a new KDR container version or documented compatible extensions.
- This first format version does **not** embed fonts, photographs, linked files or color profiles. Do not treat it as a professional archiving format yet.
- Legacy `.ocd` (plain JSON) remains supported for import and explicit export.

Tests include round-trip, corrupt checksum rejection and preserving previous saved work after a rejected save.
