# Test matrix faces

Every face here is a **subset** of a Noto face, cut down to the few characters
its row needs so the fixtures stay small and so a test can say exactly what it
expects to find. [`OFL.txt`](OFL.txt) is the SIL Open Font License 1.1 they
come under, with the copyright holders filled in.

A subset is a modified version, so the Reserved Font Name is not used: the files
are named `steleofd-*` rather than `Noto*`. The upstream project is
[noto-fonts](https://github.com/notofonts/noto-fonts); the licence permits
redistribution and modification provided the licence travels with the font,
which is what this directory is for.

| Fixture | Cut from | Kept |
| --- | --- | --- |
| `steleofd-latin-subset.ttf` | Noto Sans | ASCII, space, `e` with acute, `u` with diaeresis |
| `steleofd-cjk-subset.ttf` | Noto Sans CJK SC (face 2 of the `.ttc`) | four common ideographs |
| `steleofd-hebrew-subset.ttf` | Noto Sans Hebrew | the alphabet and a space |
| `steleofd-arabic-subset.ttf` | Noto Naskh Arabic | the alphabet and a space |
| `steleofd-nocmap-subset.ttf` | the Latin subset | everything but the `cmap` table |
| `steleofd-collection.ttc` | the CJK subset | the same face twice, as a collection |

Hinting tables (`fpgm`, `prep`, `cvt `) are dropped: nothing in this project
rasterizes glyphs, and they are most of the bytes. Layout tables (`GSUB`,
`GPOS`) are kept, so a face that needs shaping still carries the shaping data
the engine does not yet apply — that is the point of the right-to-left rows.

These are test fixtures, not a font distribution. Nothing outside the test
suite reads this directory.
