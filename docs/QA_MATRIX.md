# OpenCorel engineering acceptance matrix

This project is an independent vector editor, not a verified replacement for any specific version of CorelDRAW.

## Release gates

| Area | Required evidence |
| --- | --- |
| Windows 10/11 x64 | Automated unit tests + release compilation + manual GUI smoke test |
| Linux x64 (X11/Wayland) | Automated unit tests + release compilation + manual GUI smoke test |
| Windows 7/32-bit | Separate minimum-runtime experiment on real hardware (not yet validated) |
| File safety | Save/Load test, bad input rejection, no destruction of saved document on invalid write |
| Canvas | Rectangle/ellipse/text/curves, editing, undo/redo, zoom, grid/snap, layer visibility |
| SVG | Round-trip tests for supported subset; refusal on unsupported effects |
| PDF | Valid PDF header + human inspection of page measurements and fonts; RGB proof only |
| CDR | Test independently across versions with lawful sample files; not supported yet |
| Large maps | Test 1k / 10k / 100k objects with 2 / 4 / 8 GB RAM in release builds |
| Prepress | CMYK/ICC, PDF/X, bleed, font embedding and print scale verified with printer/typography staff; not supported |

## Recommended company test set

Provide anonymized documents with rights to use for testing:

- Original CDR files from each CorelDRAW version actively used in the company
- Poster with large text and Cyrillic fonts
- Map with many paths, layers and fine lines
- Large banner (3×2m or larger) with final PDF supplied by the print shop
- SVG/AI/EPS and bitmap files routinely received from customers
- Expected results: screenshots/renders, physical sizes, fonts, color mode and editable entities

A claim of full compatibility requires exact measurable per-version pass rates. Do not market 100% support without the relevant regression corpus.

## v0.19 Object Studio regression gates

| Scenario | Required evidence |
| --- | --- |
| Legacy object records | Deserializing a KDR object without `visible`/`locked` defaults to visible/unlocked; re-save/load preserves explicit state |
| Object visibility | Hidden object is absent from canvas, SVG/PDF export, hit testing and marquee; can still be selected and shown from Object Studio |
| Object lock | Locked object remains visible/exportable but is excluded from canvas picking, movement, node editing, color/appearance changes and batch transforms |
| Duplicate | Complete group and clip pairs preserve geometry, text format, style, appearance and links; partial group/clip selection fails without mutation |
| Stacking | Raise/lower one and front/back preserve relative order; no-op does not create an undo checkpoint |
| Layers | Rename/reorder; moving objects validates target; deleting a non-final layer transfers its objects; deleting the last layer is refused |
| Undo | Every successful manager action is one checkpoint; invalid or no-op actions do not change document/history |
| Scale | Object manager must remain virtualized and responsive with 1k / 10k / 100k objects; avoid building a widget for every object each frame |

