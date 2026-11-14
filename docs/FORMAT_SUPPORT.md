# File format support (development branch)

| Format | Import | Export | Notes |
|---|---|---|---|
| OCD | Yes | Yes | Native JSON schema v1 with backward-compatible layers |
| SVG | **Limited** | Basic | Import rect, ellipse, circle, line, polyline, polygon, text and M/L/H/V/C/S/Q/T/Z paths; rejects arcs, transforms, masks, filters, images and advanced CSS |
| CDR | No | No | Independent interoperability research required |
| CMX, AI, EPS, PDF | No | No | Planned; no compatibility claim |
| PNG, JPG, TIFF, PSD | No | No | Planned |

SVG import is deliberately strict: unsupported graphics are reported as errors instead of silently discarded. Complex art with gradients, transparency, text shaping or transforms is NOT preserved. Validate all jobs before production.
