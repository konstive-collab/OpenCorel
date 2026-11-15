# Compact OpenCorel tool rail — UI implementation and verification

## Problem

The prior 184–248 px sidebar used eight full-width, 35 px primitive buttons plus always-visible how-to tips, clipboard controls and setup notices. At narrow screen widths it removed usable drawing space and made advanced paths/text look like secondary tools. Reference: user-supplied screenshot of the left sidebar.

## Solution

- Default left rail is 64 px wide, nonresizable; label mode optionally expands to 182 px.
- Four primary tool buttons: Select, Node edit, Text, Bézier curve. Vector glyphs are drawn with egui painter; no operating-system icon font dependency, reducing missing-glyph boxes.
- Less-frequent rectangle/ellipse/line/hexagon share a flyout button. Existing F6/F7/L/P shortcuts stay intact.
- Direct access to image placement, preflight, help and compact overflow for sample/copy/paste.
- The large permanent how-to and duplicate quick-action section are removed. Per-tool hints appear via hover and the full tutorial via F1.
- Text creation and numeric node X/Y controls now appear in the Properties inspector; the tool mode/shortcut appears in the context bar.
- All menu actions, keyboard commands, core vector tools, image import and layer operations remain implemented.

## Automated acceptance

- New component tests assert the 4+4 tool grouping, 64 px collapsed default and 182 px expanded option.
- CI must run debug and release tests, Clippy, release desktop and CLI builds on Windows/Linux.
- Test that output files and saved native KDR remain unchanged: this UI-only patch does not modify document schema.

## Manual acceptance still required

- Check actual pixels/rendering on Windows 10/11 at 100%, 125%, 150% DPI and Linux.
- Verify open/close of primitive and overflow flyouts, compact/expanded transitions, keyboard-only tool switching, tooltips, node and text editing in right inspector.
- Verify main window minimum width 780 px, full HD, and large displays.
- Validate screen-reader and accessibility navigation; egui tooltips are not a replacement for full accessibility support.
- Check mouse pointer targeting and touch devices if targeted later.

This is an actual source change, not a generated screenshot mockup, and it is not production GUI certification.
