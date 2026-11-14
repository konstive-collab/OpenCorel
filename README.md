# OpenCorel — independent Rust vector editor

[Русская версия](README.ru.md) · [Format matrix](docs/FORMAT_SUPPORT.md) · [KDR specification](docs/KDR_SPEC.md) · [Contributing](CONTRIBUTING.md) · [QA criteria](docs/QA_MATRIX.md)

**Alpha v0.3 preview — NOT a fully compatible replacement for CorelDRAW.** Designed for posters, maps and large advertising sheets, currently targeting Windows 10/11 x64 and Linux x64.

- Familiar professional graphics workspace, page rulers, toolbox, layers, color palette, undo/redo, Bézier paths and grid snapping.
- **New `.kdr` (Kodi Draw)** native compressed editable project: versioned manifest, checksum and SVG preview; backward-compatible import of old `.ocd`.
- SVG and SVGZ import with a documented editable subset.
- Unsupported complex vector details can be preserved as one placed SVG artwork object, not individual editable entities.
- Experimental local LibreOffice/Poppler import adapters for CDR, CMX, PDF, AI, EPS, ODG, DXF, WMF/EMF, WPG, Visio, etc.
- Basic SVG export and RGB PDF proof export. **No CDR export, no PDF/X or CMYK, no verified Windows 7 support.**

## Build

```sh
git clone https://github.com/konstive-collab/OpenCorel.git
cd OpenCorel
cargo test --workspace
cargo run --release -p opencorel-desktop
```

## CLI examples

```sh
cargo run --release -p opencorel-cli -- new poster.kdr
cargo run --release -p opencorel-cli -- convert drawing.svg drawing.kdr
cargo run --release -p opencorel-cli -- convert source.cdr source.kdr
cargo run --release -p opencorel-cli -- export-pdf poster.kdr proof.pdf
```

For CDR/CMX and many legacy formats, the application needs a separate installation of **LibreOffice** and often **Poppler pdftocairo**. You may configure `OPENCOREL_SOFFICE` to point to the LibreOffice binary.

Import adapters are **experimental, potentially lossy, and usually single-page**. A listed extension does not mean every version is supported. **Do not delete original CDR/PDF/AI files or replace production artwork without visual and print checks.** See [the format matrix](docs/FORMAT_SUPPORT.md).

MIT-licensed independent project, not affiliated with Corel Corporation. OpenCorel is a provisional working name subject to trademark review.
