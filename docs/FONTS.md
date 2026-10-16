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
4. **A page that cannot load a face still fails as a whole.** The chain is
   now built: `Document::open` calls `FontManager::build_fallback_chain` once
   every declared face is registered, so `fallback_for` has real faces to
   consult instead of an empty list. What is still missing is the reader that
   acts on it. A run whose own face is unreadable is skipped by the SVG and
   PDF writers, but the raster backend fails the whole page, and no backend
   reports which run it dropped as a fallback rather than a substitution.
   Two smaller limits ride along: the registry is filled from the ids the
   pages actually name, so a face a document declares but never draws with
   never joins the chain, and nothing yet re-shapes a run against a fallback
   face -- the chain is available but the glyph walk does not retry on it.
   `tests/font.rs` and `tests/core.rs` pin the chain's contents and order.
   This is M2-c.
5. **No subsetting on the way out.** Embedded faces are carried whole. A
   document that embeds a 20 MB CJK collection pays for it in every conversion,
   which is M2-b.

## How a face gets from a document to a glyph

The path is short and every step is a clause:

1. **Clause 7.9** — the document's `PublicRes.xml` and `DocumentRes.xml` carry
   a `Fonts` table. Each `Font` names a resource id, a family name and a
   `FontFile`, resolved against the table's `BaseLoc`. This is the step that was
   missing for so long: the table was parsed and dropped, so every document
   silently used a system face instead.
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
