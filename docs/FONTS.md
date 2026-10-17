# The font matrix

What this engine can draw with, one row per kind of face, and — just as
importantly — what it cannot yet do. Each row is a real face under
[`crates/steleofd-font/tests/data`](../../crates/steleofd-font/tests/data) and
a test in
[`crates/steleofd-font/tests/matrix.rs`](../../crates/steleofd-font/tests/matrix.rs)
that proves the row. A row without a test is a claim, not a row.

The fixtures are subsets of Noto faces, cut down so a test can say exactly what
it expects. They carry the SIL Open Font License 1.1 and no Reserved Font
Name; see the [fixtures README](../../crates/steleofd-font/tests/data/README.md).

## The matrix

| Row | Face | Script | Proven |
| --- | --- | --- | --- |
| Latin | Noto Sans | Latin | Loads; each code point resolves to its own glyph with an outline; advances are proportional; a code point outside the face reports missing rather than drawing a wrong glyph. |
| CJK | Noto Sans CJK SC | Han | Loads; ideographs resolve and outline; every ideograph shares one advance exactly one em wide, which is what makes a column line up. |
| Right to left | Noto Sans Hebrew | Hebrew | Loads; every letter gets a glyph, nothing is dropped, clusters and source text survive. Glyphs come out in **logical** order — see the gaps below. |
| Right to left | Noto Naskh Arabic | Arabic | The same, on a cursive face whose letters join in the font's own tables, which this engine does not apply. |
| No code point map | Noto Sans without `cmap` | Latin | Loads; outlines by **glyph index**, which is what clause 11.4's `CGTransform` names; text cannot be shaped from characters, and says so. |
| Bare CFF | built in `src/cff.rs` | Latin | A face that is not sfnt at all: version 1, header size 4, offset size 2 and 3, character strings and subroutines. This is the row the 42-page scanned invoice needed. |
| Collection | Noto Sans CJK SC as a `.ttc` | Han | The first face of a collection is really read — it outlines — rather than the container being skimmed for a plausible header. |

## The gaps, stated plainly

A matrix that only lists what works is a marketing document. These are the
things a face in the table above **cannot** do yet, each with the test that
pins today's behaviour so the day it changes, the test fails.

1. **No shaping, so no visual order.** `shape_text` walks the string in order,
   asks the face for a glyph, and stops there. It does not apply `GSUB` or
   `GPOS`, so no ligature forms, no contextual alternates, no cursive joining,
   and no right-to-left reordering or bidirectional reordering. Hebrew and
   Arabic come out left to right. `a_right_to_left_face_shapes_its_letters_in_logical_order`
   asserts clusters `0..n` ascending, which is the bug as much as the promise:
   when a shaper lands, that assertion is the one that has to change.
2. **No CJK line breaking.** `ShapeOptions.language` is carried through to
   nothing. There is no line breaker, no prohibition rules, no punctuation
   compression, and no vertical writing. A CJK run is laid out as one line.
3. **Only the first face of a collection is addressable.** A text object names
   a face by resource id and an id names one file, so a `.ttc` contributes the
   face at index zero and the rest are unreachable. Producers that embed a
   collection as a single file and expect more than its first face will be
   short of glyphs.
4. **A named face that is not shipped is searched for, then reported.** When a
   document names a face and ships no `FontFile`, the declaration is kept
   (`FontSource::Unshipped`) rather than dropped, so nothing downstream has to
   guess why a page is blank. On first use the host is searched in the order
   clause 11.1's traits describe: the family name the declaration gives (only
   a family the font source really lists is selected, by exact name), the same
   family in the declared `Bold`/`Italic` shape, then the `default_cjk_family`
   and finally a generic family chosen by `FixedWidth` then `Serif`. A
   candidate is accepted only if it actually carries the characters about to be
   drawn — a generic family is a shape, not a coverage promise, and `monospace`
   resolving to a Latin-only face is how a "substitute" still produced a blank
   page. When no listed family covers the text, every listed family is tried
   before the shape alone is accepted. A run whose own face will not load gets
   one more chance through the chain (`steleofd_display::run_face_for`), and a
   code the chain cannot cover is counted, not swallowed.

   Reporting is the other half, and it is not optional: `RasterOutput` and
   `ConversionStats` both carry `missing_fonts` and `missing_glyphs`,
   `Document::missing_fonts()` exposes them to any caller, and the CLI prints
   them on stderr and exits non-zero (`Report::warn`) for `info`, `render` and
   `convert`. A face that could not be found is never described as "drawn with
   a substitute": if the text did not get drawn, the message says so.

   One limit remains by design: a build with no system fonts and nothing pinned
   (`system-fonts` off, or wasm) can find nothing, so it reports every named
   face as missing rather than drawing something unrelated. `tests/font.rs`,
   `tests/core.rs` and `crates/steleofd-parser/tests/fonts.rs` pin the chain,
   the substitution and the unshipped-declaration contract from the repo's own
   fixtures rather than a host font.

### What carries across operating systems, and what does not

The algorithm is host-independent; the answers are not, because the faces a
host installs are not. Nothing in the search is keyed to a platform API: every
step goes through `font_kit::source::SystemSource`, which font-kit implements
over fontconfig on Linux, DirectWrite on Windows and Core Text on macOS and
iOS, with the same `all_families` / `select_family_by_name` surface.

| Step | Depends on the host | Why |
| --- | --- | --- |
| Exact family name | Only the spelling | A document naming `SimSun` finds it on Windows and is reported missing on a Linux host that does not have it; the reverse holds for `Noto Sans CJK SC` |
| `Bold` / `Italic` | No | Weight and slope are compared per face, whatever the family |
| Pinned family | Yes | `Noto Sans CJK SC` is the usual Linux answer and is rarely installed on Windows or macOS, so the next row is what they take |
| CJK family list | Yes, but by name | `default_cjk_families()` names the faces each host keeps Chinese in — `Microsoft YaHei` and `SimSun` on Windows, `PingFang SC` and `Hiragino Sans GB` on macOS, `WenQuanYi Zen Hei` and `Droid Sans Fallback` on Linux — and the first that covers the text wins |
| Generic families | Yes | font-kit maps `sans-serif` to Arial and `monospace` to Courier New on Windows and macOS: Latin faces both. This is why they come last, not first |
| Coverage check | No | `FontFace::covers` reads the code points out of the face itself |

`crates/steleofd-font/tests/host.rs` pins the rules a host without the pinned
family depends on — a cached face is searched again for characters it lacks, a
run of Han is never answered by the Latin face the previous run left in the
cache, a changed pinned family clears the recorded misses — so those run on
Linux CI as well. The one thing Linux CI cannot prove is that a Windows
workstation actually picks `Microsoft YaHei`; that needs a Windows runner,
which this repository does not have yet.

The list itself is configuration, not code: see [Font families as
configuration](#font-families-as-configuration) for where it comes from and how
to change it.

5. **No subsetting on the way out.** Embedded faces are carried whole. A
   document that embeds a 20 MB CJK collection pays for it in every conversion,
   which is M2-b.

## Font families as configuration

No family name is hard-wired into the search. The names live in a [`FontConfig`]
that the manager is handed, and the engine owns only the *default* value of it.

| Layer | Where it lives | Who sets it |
| --- | --- | --- |
| Built-in default | `default_cjk_families()` in `crates/steleofd-font/src/config.rs` | The engine, once, as data |
| Host or user | Any caller holding a `FontManager` | `set_config`, or `set_default_cjk_family` for one family |
| Document | Not read yet — see below | — |

`FontConfig` has two ordered lists. `cjk_families` leads the search and
`fallback_families` trails it; neither replaces what the engine ships, so a
configured list is a preference over the default rather than a map of every host.
`with_fallback_from` layers one configuration over another for a caller that
wants a document's names over a user's, and the whole type is `serde`, so a
configuration file is a small JSON object:

```json
{
  "cjk_families": ["My Preferred CJK"],
  "fallback_families": ["My Last Resort"]
}
```

The one name that is still a string in the search code is `"cjk-substitute"`,
and it is not a host family at all: it is the *registry* id under which the
engine's own last-resort face is cached, so a caller can pin bytes to it.

**The document layer is designed, not wired.** Reading a `substitute-matching`
list out of `OFD.xml` needs a schema change first: `validate_ofd.py` checks the
root against the standard's own schema, and an element the standard does not
define would be rejected by `scripts/check_all.py`. Clause 11.1 also places the
`Font` element in a namespace a publisher cannot extend, so a reader that
accepted one would be reading a document no other reader can. Until that
decision is made, a host-level configuration is the way to name a family the
built-in list does not know.

[`FontConfig`]: https://docs.rs/steleofd-font/latest/steleofd_font/struct.FontConfig.html

## How a face gets from a document to a glyph

The path is short and every step is a clause:

1. **Clause 7.9** — the document's `PublicRes.xml` and `DocumentRes.xml` carry
   a `Fonts` table. Each `Font` names a resource id, a family name and,
   optionally, a `FontFile` resolved against the table's `BaseLoc`. Clause 11.1
   makes the file optional on purpose: the rest of the entry — `FamilyName`,
   `Charset`, `Serif`, `Bold`, `Italic`, `FixedWidth` — is what a reader matches
   a substitute against when it ships no file. An entry without one is kept as
   an unshipped declaration for exactly that purpose, not discarded.
2. **Clause 11.1** — a `TextObject` names a face by that id. The id resolves to
   the file above.
3. **Clause 11.4** — a `CGTransform` names glyph indices for a stretch of the
   text. Where it is present the run holds indices; where it is absent the run
   holds Unicode code points. A run never mixes the two, and `GlyphRun`
   carries a flag saying which it is.
4. **The face answers** — `glyph_id` through `cmap` for a code point, or
   `outline` directly for an index. The magic at the front of the file decides
   what kind of face it is; the file name never does, which matters because two
   of the three raw CFF faces in the corpus carry a `.ttf` name.

The matrix above covers steps 1 to 4 as an engine. It says nothing about how a
backend paints the outline: that is the raster and vector exporters, and their
coverage is tracked under M3 in the [roadmap](ROADMAP.md).
