# File compatibility status (development)

| Format | Open | Export | Notes |
|---|---|---|---|
| OCD | Yes | Yes | Native JSON schema v1; old files supported |
| SVG | Limited | Basic | Strict subset import (see limitations below) |
| PDF | No | **RGB proof export** | Vector graphics, up to 5 m per dimension; NOT PDF/X, no CMYK or output profile |
| CDR | No | No | Version-specific compatibility research needed |
| AI, EPS, CMX | No | No | Planned |
| Raster/PSD/TIFF | No | No | Planned |

SVG importer currently supports basic shapes, text, and M/L/H/V/C/S/Q/T/Z path curves.
It explicitly rejects many unsupported features (arcs, gradients, transforms, nested images, clipping/masks, nonstandard paints, effects) to avoid silent damage.

**Printing warning:** PDF output is presently a visual proof only. Letter spacing, font substitution, colors, clipping, bleeds, overprints and output intent have not been verified. Do not deliver it to print without an external prepress check.
