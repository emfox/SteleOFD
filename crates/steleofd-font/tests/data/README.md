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
| `steleofd-cjk-subset.ttf` | Noto Sans CJK SC (face 2 of the `.ttc`) | twelve common ideographs, including the ones the shaping and fallback tests draw |
| `steleofd-hebrew-subset.ttf` | Noto Sans Hebrew | the alphabet and a space |
| `steleofd-arabic-subset.ttf` | Noto Naskh Arabic | the alphabet and a space |
| `steleofd-nocmap-subset.ttf` | the Latin subset | everything but the `cmap` table |
| `steleofd-collection.ttc` | the CJK subset | the same face twice, as a collection |

Hinting tables (`fpgm`, `prep`, `cvt `) are dropped: nothing in this project
rasterizes glyphs, and they are most of the bytes. Layout tables (`GSUB`,
`GPOS`) are kept, so a face that needs shaping still carries the shaping data
the engine does not yet apply — that is the point of the right-to-left rows.

To rebuild the CJK subset, cut the twelve ideographs from face 2 of Noto Sans
CJK SC with `pyftsubset`, dropping the hinting tables the same way. The text is
the twelve code points U+4E2D U+53D1 U+53F7 U+56FD U+5B50 U+5E74 U+65E5 U+6708
U+7535 U+7968 U+4F60 U+597D; pass them as `--text` and keep this file ASCII.

```console
pyftsubset /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc \
  --font-number=2 --text="$(printf '\u4e2d\u53d1\u53f7\u56fd\u5b50\u5e74\u65e5\u6708\u7535\u7968\u4f60\u597d')" \
  --drop-tables+=fpgm,prep,cvt --no-hinting --desubroutinize \
  --output-file=steleofd-cjk-subset.ttf
```

These are test fixtures, not a font distribution. Nothing outside the test
suite reads this directory.
