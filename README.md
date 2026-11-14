# OpenCorel

**Independent, community-driven vector editor written in Rust**, focused on posters, maps and large-format advertising on low-end Windows and Linux workstations.

[Русский README](README.ru.md) · [Roadmap](docs/ROADMAP.md) · [Contributing](CONTRIBUTING.md) · [QA matrix](docs/QA_MATRIX.md) · [Compatibility](docs/FORMAT_SUPPORT.md)

> **Alpha / experimental software — not a full CorelDRAW replacement.** No affiliation with Corel Corporation. OpenCorel is a working title; its trademark status needs review before distribution.

## Current capabilities (v0.2 development preview)

- Familiar graphics-workspace layout: menu, properties bar, vertical toolbox, layers/objects panel, swatches, rulers, status bar and page tab.
- Rectangle, ellipse, text, polyline and Bézier editing; editable control points in object properties.
- Object ordering, duplication, undo/redo, visible/locked layers and keyboard shortcuts.
- Large page sizes specified in **millimetres**, grid, snapping and pan/zoom.
- Native open/save dialogs, OCD document storage, subset SVG import, basic SVG export.
- **RGB vector PDF proof export**, with a current 5,000 mm page dimension limit; not PDF/X and not certified for print shops.
- Cached triangulation for filled closed contours, safer temporary-file saves and stronger geometry checks.
- Automated Windows/Linux unit tests and builds, plus a CLI benchmark for document-engine performance.

## Not yet implemented

Native CDR read/write or precise cross-version CorelDRAW interoperability; AI/CMX/EPS/PSD and raster artwork workflows; multipage documents; professional font layout; verified CMYK/ICC/PDF/X print pipeline; 32-bit and Windows 7 compatibility. We need lawful CDR sample files and actual printed test jobs before any compatibility percentage can be stated.

## Build and run

Install stable Rust and Git.

```sh
git clone https://github.com/konstive-collab/OpenCorel.git
cd OpenCorel
cargo test --workspace
cargo run --release -p opencorel-desktop
```

Linux builds with GTK3 native file dialogs may require `libgtk-3-dev` plus standard X11/Wayland development packages. On Windows 10/11, release-build artifacts are uploaded by [GitHub Actions](https://github.com/konstive-collab/OpenCorel/actions) when checks succeed. These artifacts are not equivalent to a signed installer.

CLI examples:

```sh
cargo run --release -p opencorel-cli -- new poster.ocd
cargo run --release -p opencorel-cli -- inspect poster.ocd
cargo run --release -p opencorel-cli -- export-svg poster.ocd poster.svg
cargo run --release -p opencorel-cli -- export-pdf poster.ocd poster.pdf
cargo run --release -p opencorel-cli -- import-svg poster.svg copy.ocd
cargo run --release -p opencorel-cli -- benchmark 10000
```

## Contributing

Contributions are welcome in English or Russian. Read [CONTRIBUTING.md](CONTRIBUTING.md), review the [issue tracker](https://github.com/konstive-collab/OpenCorel/issues), and submit Pull Requests with tests.

The code is available under the MIT License. Do not upload proprietary source, copied CorelDRAW artwork/icons, unauthorized fonts, customer maps or confidential company documents.

This is an **independent clean-room-inspired implementation**, not a copy of CorelDRAW source code or a guarantee of file fidelity.
