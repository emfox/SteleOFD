# The clause gap survey

Why the register in [`gb-t-33190.toml`](../gb-t-33190.toml) holds the number of
entries it holds, and what is still missing from it.

## The number is not 47, and 47 was never the whole standard

GB/T 33190-2016 has far more than 47 normative clauses. What the register
tracks is not every numbered paragraph — it is every **testable, model-bearing
clause**: a clause that says something about the bytes of a document, which a
test can assert on a document the project builds. A clause that describes a
rendering intent (*"filling uses the even-odd rule"*), a term, or a paragraph of
prose has nothing to assert against and is not a register row.

So the honest statement of coverage was never *"47 of 47"*. It is *"every
clause the register lists is verified or explicitly partial"*, and it is the
register that decides which clauses those are. That distinction is what this
survey exists to make checkable rather than assumed.

## Method

The annex A schemas in [`compliance/schema/annex-a`](../schema/annex-a) are the
evidence base: the XSD files were rebuilt from the printed normative annex, so
every element, attribute and enumeration the standard defines appears in them by
name. Comparing the union of those names against the identifiers the parser and
model read is a mechanical way to find a construct the register has never
mentioned — it cannot find a *rule* that was missed, but it does find *missing
data*.

The second input is the standard's own part headings, which set the shape a
clause number must have. The register's numbering follows those.

## What the survey found

Comparing annex A element and attribute names against
`crates/steleofd-parser/src` and `crates/steleofd-model/src` turned up these
constructs with **no register entry and no test**. All are real parts of the
format; none is a rendering nuance.

| Construct | Where it is defined | Read today? | Register |
| --- | --- | --- | --- |
| `PageRes` — page-level resource table | `Page.xsd` (`ST_Loc`, `0..unbounded`) | No | now `7.10` |
| `VPreferences` — viewer preferences | `Document.xsd` (`PageMode`, `PageLayout`, `HideToolbar`, `HideMenubar`, `HideWindowUI`, `Zoom`, `ZoomMode`) | No | now `7.11` |
| `Permissions`, `Watermark`, `Print`, `Edit`, `Export`, `ValidPeriod` | `Document.xsd` | No | now `7.12` (`8.1.6` for the print-quality pair) |
| `ColorSpaces` — declared colour space profiles | `Res.xsd` (`ColorSpace`, `Palette`, `Profile`, `BitsPerComponent`) | No | now `7.13` |
| `Page/Thumbnail`, `Res/Thumbnail` | `Page.xsd`, `Res.xsd` | Page thumbnail read only as a *rendering quality* enum in `display`, not as the resource it is | now `7.14` |
| `Pattern` and `GouraudShd` fills | `Page.xsd` (`CT_Color` choice) | Parsed far enough to be **skipped** — `content.rs` steps over both | now `8.3.5`, `8.3.6` |
| `LaGourandShd` | `Page.xsd` | No | deviation, recorded |
| `8.1.1`, `8.1.2`, `8.1.4` — page/object space clauses | body of the standard | The neighbouring `8.1.3` and `8.1.5` are rows, these are not | now `8.1.1`, `8.1.2`, `8.1.4` |
| `8.2.3` — line width and miter limit | body | Implemented, no register row | now `8.2.3` |
| `8.3.1`, `8.3.3` — colour space and transparency | body | Implemented, no register row | now `8.3.1`, `8.3.3` |
| `8.6` — graphics unit base | body | Implemented, no register row | now `8.6` |
| `9.3.1` — abbreviated data, opening | body, table 36 | Implemented, no register row | now `9.3.1` |
| `9.4` — the `Path` object | body | Implemented, no register row | now `9.4` |
| `11.5` — text direction and alignment | body | Implemented, no register row | now `11.5` |
| `18.3`, `18.4` — signature sealing and validation | body | Implemented in `steleofd-sign`, no register rows | now `18.3`, `18.4` |
| `21` — OFD/A archival profile | body | `steleofd-archive` audits it, no register rows | now `21.1`, `21.2`, `21.3` |

## What is deliberately *not* a row

- **Rendering intent with nothing to assert.** *"The current point moves to
  ..."* is a sentence about a drawing model, not a byte in the file. It belongs
  in a rendering test's commentary, which is where most of them are.
- **`LaGourandShd`.** The schema carries the element and the standard describes
  it, but it appears in no document in either corpus, and the algorithm it names
  is a well-known mesh shading. Reading it into the model without a single
  sample to check against would be a claim, not coverage. It is recorded as a
  `deviation` row with the reason, so the gap is visible rather than silent.
- **Terms, abbreviations and bibliography.** Clauses 2, 3 and 4 by number; they
  define words.
- **Annexes B onwards.** Informative, and about migration between formats.

## Adding a row

A row is added when the project can point at something a document writes. The
order the rows are added in should follow the corpus: a construct no sample uses
and no producer emits is a lower priority than one that appears in a document
the project already opens. Every row added here names the construct it tracks,
states its status honestly, and — where the answer is "not implemented" — says
so as a `deviation` with the reason rather than leaving an empty `tests` list.
