# The clause register, and how its numbers are settled

What [`gb-t-33190.toml`](../gb-t-33190.toml) holds, and how every number in it
is checked against the standard.

## What a row is

GB/T 33190-2016 has far more than the register's 56 numbered clauses. What the
register tracks is not every numbered paragraph — it is every **testable,
model-bearing clause**: a clause that says something about the bytes of a
document, which a test can assert on a document the project builds. A clause
that describes a rendering intent (*"filling uses the even-odd rule"*), a term,
or a paragraph of prose has nothing to assert against and is not a register
row.

So the honest statement of coverage is not *"56 of 56 clauses of the
standard"*. It is *"every clause the register lists is verified"*, and it is the
register that decides which clauses those are. There is one status in use —
`verified` — and no `partial` or `deviation` row.

## The numbers come from the standard

Every number is read out of the standard by
[`scripts/gbt33190.py`](../scripts/gbt33190.py) rather than taken from a
reading of it. What the script reads is not the PDF but the transcription
beside it, `GBT33190-2016.zh_CN.md`, which the pinned fixtures repository
holds next to the issued document:

- The transcription was produced mechanically from the PDF by
  `gbt33190-standard/scripts/pdf_to_markdown.py`, which is kept with the text
  because its only output is the text. Assembling a heading there means
  knowing that `8.3.3` is printed as three runs on three baselines and that a
  deep number is split across four, so that knowledge stays with the
  transcriber. This repository reads the result and never needs the geometry.
- A heading is a `#` line that opens with a clause number. The transcription
  sets the depth the number has: chapter headings, then `###` for `8.3`,
  `####` for `8.3.4.1`, and `A.1 OFD.xsd` under the annex label. The hash
  count is not trusted to agree with the number for annex A, whose sections
  sit one level shallower than their numbers imply, so the depth in the output
  comes from the number itself.
- The schema listings of annex A are fenced code and are skipped, so an XML
  fragment can never be read as a heading. The same script reads them out as
  the XSD files under `compliance/schema/annex-a`.
- The contents pages are read as well -- the bulleted list under the contents
  heading, where a page number follows each title -- and the two lists are
  compared. Both readings return 108 headings: clauses 1-20, the annex A
  label, and the 13 schemas. All 94 contents entries read back with an
  identical title.
- One title of the two readings differs by a space, at `9.3.1`: the body sets a
  space before `XML`, the contents do not. The comparison is exact, so this is
  reported and is not treated as a miss.

```
$ python3 scripts/gbt33190.py --fetch clauses \
      --register compliance/gb-t-33190.toml
108 clauses
contents list 94 clauses, 94 of them read back alike
register claims 56 clauses, 56 of them in the standard
```

`--fetch` reads the transcription out of the fixtures commit, held to the
SHA-256 its manifest records; pass a path instead to read a local copy. Point
`https_proxy` at the proxy for the fetch to leave the machine -- `urllib` does
not read `ALL_PROXY`.

`--register` refuses a row whose clause the standard does not number, which is
what keeps the register from drifting away from the text.

## Where each row points

| Row | The standard | Note |
| --- | --- | --- |
| `7.9` | `7.9 (resources)` | the resource table; the last sub-clause of chapter 7 |
| `7.5` | `7.5 (document root node)` | `VPreferences` and `Permissions` are children of the root |
| `7.7` | `7.7 (page object)` | `Thumbnail` is an attribute of `Page` |
| `8.3.3` | `8.3.3 (pattern fill)` | |
| `8.3.4.1` | `8.3.4.1 (concept)` | heads the gradient family, `8.3.4.1`–`8.3.4.5` |
| `8.3.4.4` | `8.3.4.4 (Gouraud shading)` | |
| `8.3.4.5` | `8.3.4.5 (lattice-form Gouraud shading)` | |
| `8.5` | `8.5 (graphics object base)` | the last clause of chapter 8 |
| `9.1` | `9.1 (path object)` | chapter 9 ends at `9.3.5 (arc)` |
| `10` | `10` | the chapter is a single clause |
| `11.3` | `11.3 (text positioning)` | chapter 11 ends at `11.4.5 (many to many)` |
| `18.2.1`, `18.2.3` | `18.2.1 (file digest)`, `18.2.3 (signature appearance)` | chapter 18 has `18.1` and `18.2` |
| `6.2`, `7.9` | `6.2 (file organisation)`, `7.9 (resources)` | the archival container is described by these, not by a clause of its own |

Each row's tests are named `gb_t_33190_<clause with dots as underscores>_…`,
so a test name states the clause it belongs to.

## What is deliberately *not* a row

- **Rendering intent with nothing to assert.** *"The current point moves to
  ..."* is a sentence about a drawing model, not a byte in the file. It belongs
  in a rendering test's commentary, which is where most of them are.
- **Terms, abbreviations and bibliography.** Clauses 2, 3 and 4 by number; they
  define words.
- **Annexes B onwards.** Informative, and about migration between formats.
- **`PrintQuality`.** Absent from chapter 8 and from the vendored Annex A
  schema; there is nothing in a document to read.
- **Chapter 13 on its own.** A single clause with no bytes of its own to read.

## Adding a row

A row is added when the project can point at something a document writes, and
its number is taken from the extracted clause list rather than from a reading
of the standard. Run the extractor with `--register` afterwards: it refuses a
row whose clause the standard does not number, and prints any row whose title
disagrees with what the standard sets. The register's titles are English
paraphrases by design, so title differences are reported for information; a
clause the standard does not have is an error.
