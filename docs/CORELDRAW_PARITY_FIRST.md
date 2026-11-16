# OpenCorel — CorelDRAW-workflow parity first

**Product direction (approved 16 November 2026):** first make OpenCorel a credible,
fast, comfortable vector illustration and layout editor for the everyday
professional workflows commonly performed in CorelDRAW. Unique/OpenCorel-only
features are deliberately deferred until the core workflows are reliable.

This is a *workflow* target, not a promise of 1:1 feature equivalence, UI cloning,
CDR round-trip fidelity, or support for every historical CorelDRAW version.
CorelDRAW is a third-party trademark.

## First-order acceptance principles

1. A feature is **implemented** only when working interaction, persistence,
   deterministic undo/redo, tests and a built artifact exist. A label or
   unfinished command is not accepted as functional parity.
2. Every tool must be usable with a mouse/pen without unbounded frame time,
   blocking full-document cloning on each node, or silent data loss.
3. Windows and Linux are first-class; test with medium and large multi-object
   advertising projects. Report manual GUI verification separately from CI.
4. New .kdr data-model features must preserve backward readability, support
   mixed old/new documents and reject invalid geometry without corruption.
5. Formats are judged on *real files*, with an editable/imported/flattened/
   unsupported capability matrix. A successful import of one file does not
   imply general CDR or EPS support.
6. Avoid experimental AI features until core editing, text, color, output and
   recovery can pass acceptance fixtures.

## Development order (supersedes feature ordering in the older 20-step plan)

| Phase | CorelDRAW-like working scenarios | Acceptance gate |
|---|---|---|
| P0 — Editor foundations | Selection and multi-selection, move/resize/rotate/skew, hand/zoom, Bezier/Pen/Freehand, node and tangent handles, join/split/close paths, smooth/cusp/symmetry, snapping | Mouse-driven 100/1,000-node fixtures, responsive editing, stroke fidelity, undo/redo, layer locks |
| P1 — Constructive geometry | Weld/Union, Trim/Difference, Intersect, XOR, combine/break apart, compound paths and even-odd holes, contour offsets, alignment/distribution, ordering/grouping | Geometry/topology tests on crossing, nested, self-intersecting and degenerate paths |
| P2 — Text and pages | Artistic and paragraph text, real font selection and shaping, kerning/tracking, text-on-path, in-place editor, multi-page layout, templates, guides, rulers, units | Cyrillic/Latin mixed fixtures; exact selection/document save; 100-page test |
| P3 — Appearance/effects | Fills, outline widths and caps, gradients, transparency, mesh fills, blend, envelope, distortion, shadow, power clip/masks | Non-destructive editing + before/after raster/SVG/PDF golden comparisons |
| P4 — Professional output | SVG/PDF/EPS interchange, reliable bitmap import/export, bleeds/crop marks, print sizing/tiling, color-management foundations, CMYK and ICC testing | Real-world document corpus, PDF validators, print shop proofing |
| P5 — Reliability and interoperability | Autosave, crash recovery, versioned migrations, corrupted-file defense, higher-fidelity CDR import research | Recovery tests, performance budgets, compatibility matrix |
| P6 — OpenCorel-specific enhancements | Native automation/API, smart assistants and other differentiating features | Begin only after P0–P5 essential workflows are reliably usable |

## Current state (no overstatement)

- **Present but incomplete:** ordinary vector objects, cubic Bezier representation,
  node editing, basic boolean paths, clipping, transforms, gradients/opacity,
  a system-font picker and editable text (v0.14 branch). Coverage is not yet
  equivalent to mature CorelDRAW editing.
- **Known reported regression:** user testing found the Bezier creation tool
  sluggish in the Windows build. The current branch removes full-document
  checkpoints per added segment and caches adaptive path samples. These
  changes require latest-head CI and manual Windows re-test.
- **Known UX gaps:** direct drag-to-shape Bezier handles, smooth/cusp node types,
  true caret inside canvas text, advanced effects, multi-page navigation,
  comprehensive CDR fidelity and professional print/color workflows.
- **Release discipline:** avoid claiming a stable release from green CI alone.
  Distribute actual Windows test ZIP artifacts after successful checks and
  request hands-on feedback on the relevant workflow.

## Bezier-specific QA for the next Windows test build

1. Open a new document; choose Bezier; click the starting point, then add
   250 points. Confirm there are no escalating pauses per vertex.
2. Move the cursor between points; the rubber-band preview should follow the
   pointer without creating extra geometry.
3. Double-click to finish. Use Ctrl+Z once to remove the newly created path;
   Ctrl+Y should restore it as a complete path.
4. Repeat in a document with large embedded bitmap(s), and at zoom 20%, 100%,
   and 800%; note any visible lag or image flicker.
5. Edit control handles using the Nodes tool and verify that cached
   stroke tessellation is invalidated.
6. Test drawing after changing layers, and with hidden/locked layers;
   prohibited edits must not modify the document.
