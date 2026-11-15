# OpenCorel v0.9 — technical audit and regression review

Date: 15 November 2026. Scope: geometry engine, GUI state handling, KDR/OCD preservation, synthetic stress, Windows/Linux compilation.

## Method

Baseline main CI passed Windows/Linux tests and desktop/CLI builds: https://github.com/konstive-collab/OpenCorel/actions/runs/43727227246

Source review: core/lib.rs, kdr.rs, node_edit.rs; desktop/main.rs, workspace.rs and node_interaction.rs.

New integration regression corpus tests deterministic 3000-object/group reload, Cyrillic text, mixed editable art, damaged and truncated ZIP containers, unsafe archive names, compound paths with holes and 256 node-edit roundtrip sequences.

Additional Windows and Linux CI stages:
- cargo test --workspace
- cargo test --release --workspace
- cargo clippy --workspace --all-targets
- cargo build --release -p opencorel-desktop
- cargo build --release -p opencorel-cli

## Defects and fixes

| Problem | Severity | Fix and test |
|---|---|---|
| Group identifier might collide with an object ID that appears later in the list | High | Validate the complete object-ID set; regression test rejects forward collisions |
| Align/distribute could move separate members of a group or part of a locked selection | High | Reject partial group/locked layout operations; assert no empty undo entry |
| Moving a Bézier anchor could silently shift an attached handle beyond safe limits | High | Preflight dependent handles and reject the entire edit atomically |
| Native file path tested only on small happy-path examples | Medium | Add 3000-object KDR integration, damaged archive and mixed font/path tests |
| Debug tests alone cannot prove optimized build behavior | Medium | Run release test suite and static Rust analysis on Windows and Linux |

## What this audit DOES NOT verify

- Interactive GUI behavior by a human operator; no native screenshot/pixel comparison
- Real CorelDRAW CDR version corpus, editable conversion fidelity and CDR export
- CMYK/ICC, PDF/X print compliance or actual press proof
- Windows 7 launch, old 2/4/8 GB PC timings, GPU-driver diversity
- Full adversarial fuzzing and professional penetration testing
- Zero defects or 100% production correctness

Remaining architecture issues: GUI hit-testing uses bounding boxes; drag undo stores bounded full-document snapshots; massive text outlines have preview node caps. These require future iterations.

## Status

**Completed:** [PR #16](https://github.com/konstive-collab/OpenCorel/pull/16) merged to main at commit a51bf49c. The audited snapshot passed debug tests, release-mode tests, Clippy, desktop release, CLI release and artifact publishing on **Windows x64 and Linux x64**. Verified evidence: [GitHub Actions run 43735210079](https://github.com/konstive-collab/OpenCorel/actions/runs/43735210079). Both archived binaries are published as opencorel-Windows-x64 and opencorel-Linux-x64.

The initial Windows stress suite identified a test-harness mistake (attempting to overwrite a still-open NamedTempFile); corrected to write a new path and rerun successfully. This is not evidence of a product save-file bug.

The next iteration I02 v0.10 is being developed separately on branch feat/boolean-ops-v010. Production GUI, color, CDR and legacy OS validation remains explicitly pending.
