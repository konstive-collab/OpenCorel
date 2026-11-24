# OpenCorel — 20-iteration engineering plan (I01–I20)

**Baseline:** v0.8 Alpha, main commit `32f028b` (15 November 2026).
**Goal:** an independent, reliable vector editor suitable for advertising, mapping, banners and graphics production, on Windows/Linux, with an honest and testable interchange path for CorelDRAW files. This plan is a *sequence of proposed deliveries*, not a claim that features are implemented or a guarantee of CDR fidelity.

## Delivery standards for every iteration

1. Work in a separate GitHub branch and publish a PR with changed source, user-facing instructions and clear known limitations.
2. Keep native `.kdr` project files backward-readable; write migration tests before changing schemas. Avoid silent loss of editable shapes, asset bytes, font settings or layer/group structure.
3. Add unit/regression tests for new behavior, failure cases and undo/redo; run `cargo test --workspace`.
4. Require successful Windows x64 and Linux x64 release builds for desktop and CLI, with published artifacts, before a normal merge.
5. Protect performance on low-RAM computers (no unbounded caches, history or eager per-frame asset cloning).
6. Keep manually unverified behavior explicitly labelled as such. Windows 7 and professional prepress conformity need dedicated hardware and print-shop validation; generic CI does not certify them.
7. Do not misrepresent CorelDRAW trademark ownership or third-party format support. Treat CDR fidelity and export feasibility as research outcomes, not promised results.

## Iteration-by-iteration backlog

| Iteration | Planned version | Main deliverable | Minimum acceptance evidence |
|---|---|---|---|
| I01 | v0.9 | **Interactive node editor**: anchor/handle selection and drag on Bezier and polyline curves; compound glyph path controls; add/remove nodes, snap, undo | Geometry unit tests; mouse-event workflow tests; SVG/KDR round-trips |
| I02 | v0.10 | **Boolean path operations**: union, intersect, subtract, XOR on supported closed vector geometry; holes and degenerates handled | Golden shape fixtures and area/topology tests; rollback on invalid geometry |
| I03 | v0.11 | **Non-destructive clipping and masks**: editable mask objects, raster/vector clip support, export parity where feasible | Mask persistence and export comparisons; corrupted-mask rejection |
| I04 | v0.12 | **Advanced transforms**: arbitrary-angle rotation, pivot, skew, multi-object/group resize, combine/break apart | Numeric geometry tests; undo and lock-layer rejection |
| I05 | v0.13 | **Professional object appearance**: linear/radial gradients, opacity, dashes, line joins/caps, eyedropper | KDR/SVG/PDF visual baselines, stroke unit tests |
| I06 | v0.14 | **Text editing and shaping**: font picker, glyph-level shaping on canvas, accurate Cyrillic layout, text runs | Cross-platform text fixture checks and editable text preservation |
| I07 | v0.15 | **Multi-page documents**: page navigator, pages with distinct sizes, export page ranges, versioned migration | Old KDR migration and 1/100-page document tests |
| I08 | v0.16 | **Guides and smart snapping**: alignment guides, object edges/centres, ruler controls, measuring tool, units | Snap precision tests across zoom levels and large sheets |
| I09 | v0.17 | **Reusable design system**: symbols/components, named styles, saved templates, importable asset libraries | Instance/update propagation tests and portable template fixtures |
| I10 | v0.18 | **Non-destructive raster editing**: crop, alpha, placement, EXIF orientation, basic adjustments | Reopen/export parity; resolution and resource cap tests |
| I11 | v0.19 | **Vector interchange hardening**: editable SVG subset expansion, AI/EPS/PDF ingestion tests, font handling and warnings | Published format matrix and external gold-document corpus |
| I12 | v1.0 RC | **Reliability release**: autosave/recovery, unsaved-changes guard, crash diagnosis, file corruption defense | Recovery simulations, CI gating, hands-on Win10/Linux acceptance |
| I13 | v1.1 | **Color management**: ICC profile handling and CMYK data model (not yet PDF/X certification) | Color-transform test charts and reference-profile comparisons |
| I14 | v1.2 | **Standards-oriented PDF export**: PDF/X export research, transparency/font embedding and conformance validation | Independent PDF validators and print-shop proof checks |
| I15 | v1.3 | **Large-format production**: bleed/crop marks, tiling, cut contours, output presets and print layouts | Large-size page tests, production calibration samples |
| I16 | v1.4 | **CDR import R&D and broader coverage**: structural analysis of versioned files using lawful tools/fixtures | Versioned import fidelity matrix; editable-vs-placed warnings |
| I17 | v1.5 | **CDR export feasibility gate**: prototype only where lawful and technically supportable; otherwise robust KDR/SVG/PDF/CMX bridge | Go/no-go report, round-trip experiments; no false "100%" claims |
| I18 | v1.6 | **Performance on older PCs**: geometry indexing, tile rendering, memory profiling, bulk-edit responsiveness | 1k/10k/100k fixture metrics, 2/4/8 GB memory tests, Win7 feasibility assessment |
| I19 | v1.7 | **Automation/API layer**: safe batch rendering, document inspection, CLI scripting and optional plugin contracts | Determinism, sandbox boundaries, batch/export regression tests |
| I20 | v1.8 | **Deployment/acceptance**: packaging, bilingual UI, accessibility, support logs, security review, organisation rollout checklist | Installer smoke tests, production acceptance on target PCs, published support matrix |

## Scheduling, scope changes, and dependencies

Each new user request to **continue development** should begin from the next uncompleted numbered iteration; the actual status is tracked below. If an iteration uncovers a blocking architecture issue, fix and test it before adding unrelated features; record any rescheduling explicitly. We cannot implement 20 full releases asynchronously in the background from a single chat turn. Development happens during active turns or user-configured external automation, and each reported status must be grounded in GitHub commits/CI. I12 reliability work has been pulled forward as a data-loss prevention priority; I09–I11 remain on the backlog and must not be treated as completed.

No estimated dates are asserted. Capabilities marked "research", especially CDR export, PDF/X certification and Windows 7, are contingent on feasibility and external validation.

## Status ledger

| Iteration | Status | PR | CI | Notes |
|---|---|---|---|---|
| I01 v0.9 | Implemented; manual GUI/production QA pending | [#15](https://github.com/konstive-collab/OpenCorel/pull/15) | [Windows + Linux passed](https://github.com/konstive-collab/OpenCorel/actions/runs/43726921360) | Node edit, tested geometry and published desktop/CLI artifacts; merged as `58466f5` |
| I02 v0.10 | Implemented; manual GUI/production QA pending | [#17](https://github.com/konstive-collab/OpenCorel/pull/17) | [Windows + Linux passed](https://github.com/konstive-collab/OpenCorel/actions/runs/43736421574) | Boolean union/subtract/intersect/XOR, editable compound result, holes and undo; merged as `4feef2f` |
| I03 v0.11 | Implemented in source; manual export/alpha-mask QA remains | [#19](https://github.com/konstive-collab/OpenCorel/pull/19) | historical CI pending verification | Vector clipping, KDR, SVG/PDF and geometry-aware hit test |
| I04–I11 | Backlog / partial delivery | — | — | Advanced transforms, appearance, text, pages and guides exist in later source versions; symbols, raster editing and interchange hardening still need explicit acceptance against this plan |
| I12 v1.0 RC | Implemented in source; manual crash/restart and UI QA pending | [#36](https://github.com/konstive-collab/OpenCorel/pull/36) | [Windows + Linux passed](https://github.com/konstive-collab/OpenCorel/actions/runs/45440702114) | Atomic KDR snapshots, startup recovery prompt, destructive-action guard, panic reports; manual recovery simulation still required |
| I13–I20 | Planned | — | — | Not implemented |
