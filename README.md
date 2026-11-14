# OpenCorel

Experimental independent vector editor in Rust for large-format posters, advertisements and maps.

**Prototype only:** not a CorelDRAW replacement. `.cdr` import/export, print-ready CMYK/PDF, Win7 compatibility, and professional typography are NOT implemented.

Initial Rust workspace includes a platform-neutral document model, desktop GUI, and CLI. See `README.ru.md`, `docs/ROADMAP.md`, and `docs/FORMAT_SUPPORT.md` after the source import.

Build on Windows 10/11 or Linux with Rust and system GUI dependencies:

```bash
cargo test --workspace
cargo run --release -p opencorel-desktop
```

This source was prepared without a Rust compiler; builds are not yet verified. Project name is internal and should be reviewed for trademark issues before redistribution.
