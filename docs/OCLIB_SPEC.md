# OCLIB — OpenCorel Symbol Library v1

OCLIB is a portable JSON container for reusable vector-symbol definitions. The file extension is `.oclib`. It is independent from the KDR project container and can be imported into more than one document.

## Top-level structure

```json
{
  "format": "opencorel-symbol-library",
  "version": 1,
  "symbols": [
    {
      "id": 1,
      "library_key": "18c4a2d0-1234-1",
      "name": "Brand mark",
      "category": "Identity",
      "objects": []
    }
  ]
}
```

Each symbol object stores native OpenCorel geometry, style, text format, optional appearance and a display name. The exact fields for each `Shape` follow the KDR schema. No raster preview is required: the resource itself contains the editable source objects.

## Identity and import behavior

- `id` is a document-local numeric identifier. It is not globally unique and is remapped when an asset is imported into a document.
- `library_key` is the stable identity used to match an asset across OCLIB files and documents.
- Importing a key already present in a document updates that definition and propagates the change to its linked instances.
- Importing a new key allocates a fresh document-local ID.
- Updating a library entry does not silently modify already-open documents. Import or insert the updated asset into a document to apply the update there.

## Safety limits

| Limit | Value |
|---|---:|
| OCLIB file size | 64 MiB |
| Symbols per library | 512 |
| Objects per symbol | 500 |
| Embedded source data per symbol | 8 MiB |
| Embedded symbol definitions per KDR document | 32 MiB |
| Name | 1–128 printable bytes |
| Category | 0–64 printable bytes |

The reader validates the format/version, stable keys, symbol names/categories, every embedded object via the native document validator, and the resource budgets. Invalid JSON or invalid geometry is rejected; it is never silently rewritten.

## Atomic persistence

The application writes a temporary file in the destination directory, flushes it, and atomically persists it over the previous library. A failed write leaves the last valid file untouched. A corrupt local library is not automatically overwritten at startup; the user can restore it or import a backup.

## KDR relationship

KDR stores document-local symbol definitions, linked-instance records, and ordinary editable objects. The additive fields have serde defaults, so older KDR documents without symbol metadata remain readable. The KDR schema version remains unchanged in v0.23.

A symbol instance is represented by ordinary `Object` entries sharing a group ID, plus a stable instance record mapping ordered object IDs to the definition. This avoids introducing a new `Shape` variant and keeps existing SVG/PDF exporters working: those formats receive normal vector geometry, not an external library dependency.

## Current alpha limitations

- Nested symbols are not supported.
- Node editing inside a linked group is not supported; detach the instance first.
- Updating a definition scales the replacement geometry to each instance's current bounding box. It preserves position and frame, not arbitrary per-object overrides.
- Clipping relationships are not accepted when creating a symbol.
- OCLIB stores source resources, not rendered thumbnails or font files.
