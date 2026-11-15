# OpenCorel v0.10 — adversarial QA and debug pass

Scope: the audited v0.9 editor and v0.10 Boolean operations (I02 of the 20-iteration plan).

## Evidence and executed automated checks

The pre-v0.10 hardening audit is in [QA_AUDIT_V09.md](QA_AUDIT_V09.md); the audited v0.9 snapshot passed Windows/Linux debug tests, optimized tests, Clippy, desktop builds, CLI builds and artifact publication at [run 43735210079](https://github.com/konstive-collab/OpenCorel/actions/runs/43735210079).

The initial v0.10 feature PR #17 passed the same two-platform automated release gate at [run 43736421574](https://github.com/konstive-collab/OpenCorel/actions/runs/43736421574).

### Regression families

| Family | Coverage |
|---|---|
| Geometry and shapes | Rectangle overlaps and invariants, ellipse approximation error, no-overlap, containment with hole, disconnected islands, same-shape subtraction |
| Robustness at boundaries | Shared edge, corner touch, self-crossing closed path, invalid NaN and infinity, oversized coordinates |
| Error atomicity | No source mutation on invalid operands; no empty-result data loss; blocked group and locked layer refuse edit |
| Native-file compatibility | KDR/OCD, 3000-object and mixed-content reload, Cyrillic text, complex paths |
| Untrusted input | Damaged/truncated ZIP, unsafe archive paths; invalid and missing file rejection in release CLI smoke |
| Interactive model | Node insert/delete/move, exact cubic subdivision, handle validation, undo of editor actions |
| Runtime modes | Debug tests + release-mode tests + Clippy + desktop/CLI release builds on Windows x64 and Linux x64 |
| CLI round trip | Create native file → inspect → export SVG → import SVG as native project → inspect |
| Performance safeguards | Caps on Boolean complexity, node preview count and contour depth (not a throughput benchmark) |

PR #18 adds an explicit panic boundary across **all** Boolean geometry conversion and operations, since compound input reconstruction may itself perform topology work before the final set operation. It also adds the degenerate-contact and corrupted-input regression cases. **Validated and merged:** [PR #18](https://github.com/konstive-collab/OpenCorel/pull/18) passed both Windows and Linux CI, including debug and release tests, Clippy, desktop and CLI release builds, end-to-end KDR/SVG/KDR CLI smoke checks, and rejection of missing/corrupt KDR input. [Verified run 43737625143](https://github.com/konstive-collab/OpenCorel/actions/runs/43737625143). Both artifacts were published, and the changes merged into main as `e568a278`.

### Not yet tested (remain open)

- Native GUI smoke tests by an operator, selection/highlighting and actual visual render comparisons
- Real CDR corpus by file version, artwork fidelity and round-trip editability; no editable CDR export
- Windows 7 / 32-bit startup; older GPU drivers and machines with 2–4 GB RAM
- Professional PDF/X / CMYK print-shop proofs, ICC calibration and raster-text fidelity
- Exhaustive property-based fuzzing of arbitrary multi-million-segment geometry
- Long-duration crash/leak profiling and external security penetration testing

**Conclusion:** automated testing covers an expanded set of realistic correctness and failure scenarios, but neither a passing pipeline nor this audit certifies 100% correctness. Release readiness for customer print production still depends on production fixtures and manual acceptance.
