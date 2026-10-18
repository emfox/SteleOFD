# GB/T 33190-2016 annex A schemas (normative)

The 13 XSD files in this directory were **rebuilt from the printed annex A of
the standard**, not taken from an official machine-readable release.

## Provenance

| Item | Value |
| --- | --- |
| Standard | GB/T 33190-2016, fixed layout documents |
| Location | Annex A (normative) Schema, A.1 to A.13 |
| Rebuild script | `scripts/gbt33190.py schemas` |
| Input | `GBT33190-2016.zh_CN.md`, the transcription of the standard as published PDF |

The schemas are read out of the transcription rather than off the printed page.
Annex A is set staircase-indented, and glyphs such as `<`, `/`, `:` and `>` are
drawn with a bounding box about 3.6pt taller than the letters, so turning the
page into text means clustering words by the centre of their bounding box,
restoring the spaces the extractor drops inside long runs, and splitting
`elementname=` back into two tokens. That work belongs to the transcription,
and is done by `gb-t-33190-text/scripts/pdf_to_markdown.py` alongside the text
it produces.

```
python3 scripts/gbt33190.py --fetch schemas --out compliance/schema/annex-a   # rewrite
python3 scripts/gbt33190.py --fetch schemas --check compliance/schema/annex-a # verify
```

`--check` compares the XML tree of each file with the one the transcription
carries, so a difference in how the tags are laid out is not a difference in
the schema. Both commands report all 13 files.

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

The comparison was made by rendering the annex pages, OCRing them, and
requiring the printed page to carry our spelling and the wiki's variant zero
times; all eleven pairs passed. The result is that no change followed from the
comparison — an independent transcription is now on record as disagreeing, and
we are on the standard's side.

## Trustworthiness

- All 13 files build under `lxml.etree.XMLSchema()`, so they are **valid
  schemas**, not merely well-formed XML: type references and element content
  models all resolve.
- Checked clause by clause against the body of the standard: `OFD.xsd`,
  `Document.xsd`, `Version.xsd`, `Annotations.xsd` and `Page.xsd`
  (`CT_Path`, `CT_Text`, `CT_Layer`, `CT_Color`).
- **Not yet** checked attribute by attribute: `Res.xsd`, `Signature.xsd`,
  `CustomTags.xsd`, `Extensions.xsd`, `Attachments.xsd`, `Annotation.xsd`.
  What they have since been exercised against is recorded under
  `compliance/fixtures/features/`.

Treat these files as a validation baseline derived from the normative annex.
They are not third-party certification, and any conformance claim built on
them should say so.

## Usage

```console
python3 scripts/gbt33190.py --fetch schemas --check compliance/schema/annex-a
python3 scripts/pack_fixtures.py --check                  # archives match sources
python3 scripts/validate_ofd.py --allow-foreign \
    $(find compliance/fixtures -name '*.ofd' -o -name '*.xml')
```

`scripts/validate_ofd.py` routes every XML entry of a container to the schema
that declares its root element and validates it. CI runs that over everything
under `compliance/fixtures/`.

Two details are worth knowing:

- A container may carry XML in **vocabularies of its own** — the file a
  `CustomTag` points at, for instance. `--allow-foreign` accepts those; without
  it they fail loudly, which is what we want for a genuinely misplaced annex A
  entry.
- `compliance/fixtures/<name>/` is the readable source and `<name>.ofd` is
  the archive built from it. `scripts/pack_fixtures.py` reads the nesting from the
  `layout.txt` beside each source, and its `--check` mode (run in CI) fails when
  the committed archive no longer matches its sources.
