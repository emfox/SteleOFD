# GB/T 33190-2016 annex A schemas (normative)

The 13 XSD files in this directory were **rebuilt from the printed annex A of
the standard**, not taken from an official machine-readable release.

## Provenance

| Item | Value |
| --- | --- |
| Standard | GB/T 33190-2016, fixed layout documents |
| Location | Annex A (normative) Schema, A.1 to A.13 |
| Rebuild script | `scripts/extract_schema.py` |
| Input | The standard as published PDF |

Annex A is set staircase-indented. Glyphs such as `<`, `/`, `:` and `>` are
drawn with a bounding box about 3.6pt taller than the letters, so grouping
words by their top edge splits a printed line in two and turns
`</xs:sequence>` into `</:>` plus `xssequence`. The script clusters words by
the **centre** of their bounding box instead: punctuation and letters then sit
about 1.4pt apart while rows are 15.7pt apart, which separates cleanly. Each
row is ordered by left edge, and an XSD/OFD vocabulary splits the runs the
extractor had glued together, such as `elementname=` or `schemaxmlns=`.

## Known printing defect

Annex A.6 is printed as `Definition.xsd`, but A.1 to A.3 include it as
`Definitions.xsd`. This directory uses `Definitions.xsd` so the set resolves.

## Trustworthiness

- All 13 files build under `lxml.etree.XMLSchema()`, so they are **valid
  schemas**, not merely well-formed XML: type references and element content
  models all resolve.
- Checked clause by clause against the body of the standard: `OFD.xsd`,
  `Document.xsd`, `Version.xsd`, `Annotations.xsd` and `Page.xsd`
  (`CT_Path`, `CT_Text`, `CT_Layer`, `CT_Color`).
- **Not yet** checked attribute by attribute: `Res.xsd`, `Signature.xsd`,
  `CustomTags.xsd`, `Extensions.xsd`, `Attachments.xsd`, `Annotation.xsd`.

Treat these files as a validation baseline derived from the normative annex.
They are not third-party certification, and any conformance claim built on
them should say so.

## Usage

```console
python3 scripts/extract_schema.py <standard.pdf>          # regenerate
python3 scripts/validate_ofd.py compliance/corpus/minimal.ofd
```

`scripts/validate_ofd.py` routes every XML entry of a container to the schema
that declares its root element and validates it. CI runs that over everything
under `compliance/corpus/`.
