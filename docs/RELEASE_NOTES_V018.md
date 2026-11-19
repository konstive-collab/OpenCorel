# OpenCorel v0.18 — Studio / Production Mega Update

Release scope: a professional workspace iteration with document-level
functional improvements, not only visual changes.

## Delivered source
1. Stable KDR-stored guides per independent page: ruler clicks, exact edit,
   removal and validation; guides are editor-only.
2. Intelligent snapping (guide, page bounds/center, object bounds/center, grid)
   with a safety cut-off for huge object counts.
3. Complete layer management and partial z-stack ordering, atomic undo.
4. Copy/move groups, clipping links and appearances between different pages.
5. True combined multi-page RGB PDF book with unique renumbered PDF object
   references, page tree, resources and independent validation.
6. Direct vector tool glyphs for eight editing tools and 19 procedural shapes
   accessible from a compact flyout.
7. Searchable Ctrl+K command palette (35+ editing/navigation/export actions).
8. Publication-wide preflight with page-specific diagnostics.
9. CLI: export-pdf-book, page listing and batch exports.
10. Regression coverage for per-page serialization, transfers, guide edits,
    order/layer changes, PDF page count, PDF third-party parser, Windows/Linux.

## Not delivered / explicitly unverified
- Full CDR editing and native CDR export
- Typography parity, variable/OpenType features and advanced text-on-path
- CMYK, ICC proofing, PDF/X prepress compliance
- Reusable master pages, infinite undo, nested group structures
- Windows 7 runtime certification
- Direct hardware control/verification with scanners/plotters/printers/tablets
- Real-world performance benchmarks and fidelity testing on 200+ object
  imported CorelDRAW projects

## Test gate
GitHub Actions workflow: cargo test --workspace, release-mode tests,
cargo clippy, desktop/CLI builds, KDR and device format smoke checks,
PDF merged-book validation via independent pypdf, DXF via ezdxf.
Manual UX acceptance remains outstanding.

Build/run: `cargo run --release -p opencorel-desktop`
