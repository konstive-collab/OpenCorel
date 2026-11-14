# OpenCorel file compatibility — v0.3 development

**Legend**: Native = full editability within OpenCorel's currently implemented primitives; Editable subset = supported shapes/curves converted to native objects; Placed artwork = visible as a single moveable/scalable vector object, internal curves/text not editable; Adapter = a separately installed converter is needed.

| Extension | Open/import | Save/export | Dependencies / caveats |
|---|---|---|---|
| **KDR** | Native | Native | Kodi Draw v1 ZIP, JSON document, SVG preview, SHA-256 integrity |
| **OCD** | Native (legacy) | Native (legacy) | Older JSON document file |
| **SVG** | Editable subset; otherwise placed artwork | Basic SVG | SVG gradients/transforms/masks/complex paths not broken into editable shapes |
| **SVGZ** | Editable subset; otherwise placed artwork | Basic SVGZ | Gzip compressed SVG |
| **PDF** | Placed artwork (first page) | RGB vector preview | Requires `pdftocairo`; no PDF/X, not CMYK |
| **CDR** | Adapter, experimental | **No** | LibreOffice Draw import to PDF, then `pdftocairo`; compatibility depends on CDR version & filters |
| **CMX** | Adapter, experimental | No | LibreOffice Draw |
| **AI** | Adapter, experimental | No | PDF-compatible AI or LibreOffice Draw |
| **EPS, PS** | Adapter, experimental | No | LibreOffice Draw / `pdftocairo` |
| **ODG, FODG** | Adapter, experimental | No | LibreOffice Draw |
| **DXF** | Adapter, experimental | No | LibreOffice Draw; CAD units and text may differ |
| **WMF, EMF, WPG** | Adapter, experimental | No | LibreOffice Draw; visual fidelity not guaranteed |
| **VSD, VSDX, SXD** | Adapter, experimental | No | LibreOffice Draw |

A format listed under an adapter is a **conversion route, not a proven fidelity guarantee**. Some individual files may be refused. Converted drawings preserve an SVG representation as one placed object, not the original application’s editing model. The original input file is not embedded in KDR; keep your original CDR/PDF/AI files.

**Operating-system dependencies**:
- Windows/Linux native KDR/OCD/SVG/SVGZ require no office suite.
- CDR and most legacy vector formats require a compatible local LibreOffice install. Set `OPENCOREL_SOFFICE` if the launcher cannot locate it.
- PDF/AI PDF extraction and most complex import adapters require Poppler `pdftocairo` available in PATH.
- The first page only is currently imported from PDF and multipage source formats.

**Security**: conversion of untrusted files is executed by external applications under the current user's permissions, not a hardened sandbox. Test with anonymized, trusted documents and keep converter packages updated.

**Validation**: Full CDR support must be tested on real customer-owned documents from every CorelDRAW version in use, with comparisons against the original in CorelDRAW. This has NOT been performed and cannot be claimed to be 100%.
