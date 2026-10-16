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

## Cross-check against a third-party transcription

The `easyofd` wiki republishes annex A as text. Comparing its names against
these files turned up **eleven differences, every one of them the wiki's**:
`Seqment` for `Segment` (A.13), `Sianature`/`SiqnedInfo`/`Siqnatures` for
`Signature`/`SignedInfo`/`Signatures` (A.7, A.8), `OuadraticBezier` for
`QuadraticBezier` and `Pointl` for `Point1` (A.6), `RotationAnglet` for
`RotationAngle` (A.5), `Deltay` for `DeltaY` and `y` for `Y` (A.13),
`DefaultCs` for `DefaultCS` (A.2) and `Backqround` for `Background` (A.13).
They are all `g` → `q`, a case flip, or a spurious trailing letter, and the
wiki contradicts itself: its own body tables spell `Segment`, `DeltaY` and
`DefaultCS` correctly, so its XSD pages are the outlier.

The printed standard settles it, page by page and glyph by glyph. Its annex A
renders under `pdftoppm -r 400` as legible type, so each disputed name can be
read directly rather than inferred: `DefaultCS` and `Background` on the
printed page 94 (PDF 101), `QuadraticBezier` with `Point1`/`Point2`/`Point3`,
`CubicBezier` and `RotationAngle` on the printed page 108 (PDF 115),
`Segment` and `DeltaY` on the printed pages 120 and 124 (PDF 127 and 131), and
`Signature`/`Signatures`/`SignedInfo` on the printed pages 109 and 110 (PDF
116 and 117). Every one carries our spelling; none carries the wiki's.

`scripts/cross_check_schema.py` automates the same comparison: it OCRs the
annex and, for each disputed name, requires the printed page to carry ours and
to carry the wiki's variant zero times. All eleven pairs pass. The result is
that no change followed from the comparison — an independent transcription is
now on record as disagreeing, and we are on the standard's side.

## Trustworthiness

- All 13 files build under `lxml.etree.XMLSchema()`, so they are **valid
  schemas**, not merely well-formed XML: type references and element content
  models all resolve.
- Checked clause by clause against the body of the standard: `OFD.xsd`,
  `Document.xsd`, `Version.xsd`, `Annotations.xsd` and `Page.xsd`
  (`CT_Path`, `CT_Text`, `CT_Layer`, `CT_Color`).
- **Not yet** checked attribute by attribute: `Res.xsd`, `Signature.xsd`,
  `CustomTags.xsd`, `Extensions.xsd`, `Attachments.xsd`, `Annotation.xsd`.
  What the corpus has since exercised against them is recorded under
  `compliance/corpus/features/`.

Treat these files as a validation baseline derived from the normative annex.
They are not third-party certification, and any conformance claim built on
them should say so.

## Usage

```console
python3 scripts/extract_schema.py <standard.pdf>          # regenerate
python3 scripts/cross_check_schema.py <standard.pdf>      # settle a spelling
python3 scripts/pack_corpus.py --check                    # archives match sources
python3 scripts/validate_ofd.py --allow-foreign \
    $(find compliance/corpus -name '*.ofd' -o -name '*.xml')
```

`scripts/validate_ofd.py` routes every XML entry of a container to the schema
that declares its root element and validates it. CI runs that over everything
under `compliance/corpus/`.

Two details are worth knowing:

- A container may carry XML in **vocabularies of its own** — the file a
  `CustomTag` points at, for instance. `--allow-foreign` accepts those; without
  it they fail loudly, which is what we want for a genuinely misplaced annex A
  entry.
- `compliance/corpus/<name>/` is the readable source and `<name>.ofd` is the
  archive built from it. `scripts/pack_corpus.py` reads the nesting from the
  `layout.txt` beside each source, and its `--check` mode (run in CI) fails when
  the committed archive no longer matches its sources.
