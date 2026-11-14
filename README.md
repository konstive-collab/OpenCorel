# OpenCorel

**Experimental Rust vector graphics editor for posters, maps and large-format advertising.**

[Русская версия](README.ru.md) · [Roadmap](docs/ROADMAP.md) · [Contributing](CONTRIBUTING.md) · [File formats](docs/FORMAT_SUPPORT.md)

OpenCorel is an independent community-driven Rust experiment designed to explore low-resource, cross-platform vector drawing workflows for print shops and design departments.

> **Not production ready.** No CDR/CMX/AI/PDF import or export, no print-ready CMYK/PDF/X and no validated Windows 7 support yet. No affiliation with Corel Corporation.

## Present capabilities

- Workspace with independent document core and native desktop GUI.
- Rectangle, ellipse and text objects, object selection, drag to move and property changes.
- Millimetre-based sheet dimensions (including large advertising banners).
- OCD (experimental JSON) open/save, basic SVG export.
- CLI inspector, document example, test suite and GitHub Actions builds for Windows and Linux.

## Run

Install stable Rust. On Linux, install graphics/windowing development libraries for eframe.

~~~sh
cargo test --workspace
cargo run --release -p opencorel-desktop
cargo run -- -h
~~~

CLI example:

~~~sh
cargo run -p opencorel-cli -- new poster.ocd
cargo run -p opencorel-cli -- inspect poster.ocd
cargo run -p opencorel-cli -- export-svg poster.ocd poster.svg
~~~

The GUI's file field accepts local file paths. It is a functional technical prototype, not a mature graphic design workflow.

## Platforms

Windows 10/11 x64 and Linux x64 are primary targets. Windows 7 compatibility requires a separate toolchain/renderer evaluation and on-device testing. Linux support does not mean any distribution is tested.

## Help build it

Contributions are welcome in English or Russian. See [CONTRIBUTING.md](CONTRIBUTING.md), [issues](https://github.com/konstive-collab/OpenCorel/issues), and [workflow runs](https://github.com/konstive-collab/OpenCorel/actions). Please do not upload proprietary code, fonts, commercial assets, or confidential customer designs.

The source is offered under the MIT License. **OpenCorel is a temporary working name:** trademark clearance and possible rebranding are needed before distribution.
