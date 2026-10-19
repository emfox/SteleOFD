# Reader UI Layout and Frame Flow

This is the contract every reader surface follows. It exists because the reader
has three layers that can each be written in any order, and without a rule the
window turns into one long function whose behaviour nobody can test.

The rule the rest of this document serves: **the window draws, the state
decides.** `main.rs` may only read state and ask `App` to change it; `App` may
never know a widget exists. Everything testable therefore lives in `app.rs` and
`viewer.rs`, and the tests never open a window.

## 0. The guideline this follows

The window is built on the **GNOME Human Interface Guidelines**, in the edition
that documents GTK 4 and Libadwaita. That is the public document a contributor
can look up when this one is silent, and where the two disagree this one says
why. The guideline is cited by requirement, not by paragraph number, because
the online edition is a wiki and its numbering moves.

What the window takes from it, and where each requirement lands:

| Requirement | Guideline source | Where it lives here |
| --- | --- | --- |
| Few, grouped controls in a header bar; title left, main action right; disabled rather than hidden | Patterns · Header bars | §2 Z1–Z2 |
| Widgets keep a stable position as state changes, so a reader's muscle memory holds | Patterns · Header bars | §2, §4 |
| Three theme choices — follow the system, light, dark — never only two | Guidelines · UI styling | §2 Z9, §5 |
| A high-contrast mode exists and is tested, because a theme that only adjusts hues fails the readers who need it | Guidelines · UI styling | §2 Z9, §5 |
| Style through named tokens rather than literals, so every theme adapts | Guidelines · UI styling | §5 |
| A small set of text styles, named by role: body, heading, title-1 to title-4, caption, large-title | Guidelines · Typography | §5 |
| No hardcoded point sizes; a size is derived from the base and the role's ratio | Guidelines · Typography | §5 |
| No italics for emphasis and no all-caps for headings; emphasis is weight or colour | Guidelines · Typography | §5 |
| Colour is never the only cue; a state also carries a word or a shape | Guidelines · Accessibility | §2 Z6, §4 |
| Nothing blinks or pulses | Guidelines · Accessibility | §4 |
| Every control is reachable from the keyboard and shows what it does on hover | Patterns · Header bars, Reference | §2 Keyboard, §4 |

Two requirements the guideline states for a shell of its own are deliberately
not taken, and the reason matters: the window has **no header bar and no
libadwaita**, because it is drawn by `egui` on Windows, macOS, Linux, Android and
the browser alike, and a GNOME-only widget set would leave four of those five
with a window glued together from two toolkits. Z1 and Z2 are this window's
header bar, and the clauses above are applied to them.

## 1. Layers

| Layer | File | May know about | May not |
| --- | --- | --- | --- |
| Engine | `steleofd-core`, `steleofd-render` | documents, pixels | the reader |
| State | `crates/steleofd-reader/src/app.rs` | the engine, i18n | `egui`, widgets, pixels |
| Worker | `crates/steleofd-reader/src/render_worker.rs` | the engine, channels | `App`, `egui` |
| Cache | `crates/steleofd-reader/src/viewer.rs` | the engine, `ColorImage` | `App`, layout |
| Window | `crates/steleofd-reader/src/window.rs` | `App`, `egui` | the engine internals |

`main.rs` opens no file and parses nothing. It receives bytes or a path, hands
them to `App`, and paints whatever comes back. If a widget needs a value `App`
does not expose, `App` gains a method — the widget does not reach around it.

The worker exists so that rasterizing never blocks a frame. It owns its own
copy of the document, renders one request at a time, and hands results back
over a channel; `App` drains that channel once per frame, before anything is
drawn. It never touches `egui`, and `App` never waits on it.

## 2. Layout zones

The window is a stack of fixed zones around one growing canvas. Zones are
declared in this order every frame; `egui` reserves space top-down, so the order
is the layout.

```
┌────────────────────────────────────────────────────────┐
│ Z1  header          fixed    open · close · panels      │  Panel::top("menu")
│                              export · language          │
├────────────────────────────────────────────────────────┤
│ Z2  header          fixed    paging · fit · zoom        │  Panel::top("toolbar")
├────────────────────────────────────────────────────────┤
│                                                        │
│ Z3  canvas        growing    the page, or the drop     │  ScrollArea
│                 hint, or a failure                     │  id_salt("canvas")
│                                                        │
├────────────────────────────────────────────────────────┤
│ Z4  status bar    fixed      document · page · error   │  Panel::bottom("status")
└────────────────────────────────────────────────────────┘

                        Z5  search      fixed width      the query, the hits   Panel::left("search")
                        Z6  signatures  fixed width      the verdict, per sig   Panel::left("signatures")
                        Z7  annotations fixed width      the marks, by page    Panel::left("annotations")
                        Z8  comparison  fixed width      the pages that moved  Panel::left("comparison")
                        Z9  settings    menu on Z1       theme · type · contrast
```

**Z1 header.** Document-level actions only: open, close, the panel toggles, and
the export actions. Anything that acts on the current page belongs in Z2. The
close button is disabled when nothing is open, so the bar never has to explain
itself.

Three groups, as the header-bar pattern asks: the actions that change the
document on the left, the toggles that change what is shown in the middle, and
the actions that write a file or change the interface on the right. The order
inside each group is fixed, so a control the reader learned stays where it was
as the document comes and goes. A control that cannot act is disabled and keeps
its width; nothing is hidden and nothing moves.

**Z2 header.** Actions on what is on screen: previous, next, fit width, fit
page, zoom. Disabled controls rather than hidden ones, so the bar does not
reflow as the document changes. The page controls sit on the left and the zoom
slider on the right, which is the same split Z1 uses and the one place in the
window a value is dragged rather than clicked.

Every control in Z1 and Z2 says what it does in a tooltip. The guideline asks
it of header-bar buttons; this window asks it of all of them, because the
symbols it draws (`◀`, `▶`) are not words and a reader meeting them for the
first time has nothing else to go on.

**Z3 canvas.** Exactly one of five states, decided in this order:

| State | Condition | Shown |
| --- | --- | --- |
| Empty | no document open | the drop hint, centred |
| Failed | the page would not rasterize | `reader-error-label` |
| Blank | the page has no visible ink | the page, plus the blank notice |
| Pending | the page has not landed yet | whatever is cached, so far |
| Shown | otherwise | the page |

The blank case still paints the page. A blank page in a document is content,
not an absence, and hiding it would break the sense of place.

`Failed` says only that something went wrong; the engine's own message belongs
to Z4, and putting the same sentence in two places would show it twice.
`Pending` is not an error and not a blank: rendering is off the draw path, so
a page on its way is simply absent from the cache for this frame. It draws the
same thing as `Blank` and `Shown` — the pages that have arrived — which is why
the three share a branch. Z3 is still the only zone that rasterizes; it asks
for what is missing and paints what it has.

**Z4 status bar.** Left: the open document's name and page position. Right: the
zoom slider. On failure the message takes the left half and stays until it is
dismissed or the next document opens — a message that flickers away is a message
that will be missed.

**Z5 search panel.** Present only while it is open. A query box, previous and
next buttons, the result count, and the hits as a list. `reader-no-match` shows
only when a real search found nothing — an empty query, or a query made with no
document open, is not a failed search and does not accuse the reader of one.
Selecting a hit turns to its page, because a hit the reader cannot see is not a
hit they found. The canvas outlines every hit on the pages it draws, with the
current one stronger than the rest, so "where am I" and "where are the others"
are both answerable at once. It is drawn before the canvas, so the canvas
measures the space that is actually left for pages.

Every zone draws a separator line it does not own. `Panel::show_separator_line`
does this, and it is why zones are panels rather than stacked `horizontal`
closures.

**Z6 signatures panel.** Present only while it is open. The count, then one
row per signature carrying its verdict, signer, signing time and algorithm.
Four verdicts, not two: `Valid`, `Tampered`, `Unverified`, and `Untrusted`. The
engine can only conclude that a signature is good or bad, and reports a
signature it has no certificate for as an error; flattening that into "tampered"
tells the reader their document was altered when nothing says it was. Unverified
is the honest answer, and it is shown in amber so it is not mistaken for either
verdict.

The verdict is always a word, and each verdict also carries a symbol and a
weight. The guideline forbids colour as the only cue, and this panel is the one
place in the window where a colour carries a judgement; a reader who cannot
tell amber from red still reads "unverified".

Verification runs on the document once, the first time the panel opens, not per
signature — asking the engine per signature re-reads the package each time.

Like Z5, it is drawn before the canvas so the canvas measures the width that is
actually left for pages.

**Export.** Two actions at the far end of Z1: as one PDF, or as one PNG per
page. They are document-level rather than page-level, so they sit in Z1 and not
in Z2. Both write the pages the reader is currently looking at — a spread
exports as the two pages shown — because "export what I am looking at" is the
question a reader in front of a document actually has.

The window cannot know where a file should go, so it writes the request into
`App` and reports the format in `WindowRequests`; the host consumes it and asks
the platform. A PDF is one file, so the platform names it; PNG writes one file
per page, so the platform names a folder. Cancelling is not a failure.

Conversion runs on its own thread, for the same reason rasterizing does: a PDF
of a long document is written in one go and would otherwise drop frames. Unlike
a render, an export is never superseded — the reader asked for a file, and the
file either appears or it does not.

**Z9 settings.** Three choices at the right end of Z1, each a control rather
than a dialog: the theme, the text size, and high contrast. They are settings
about the window, not about the document, so they survive closing a document and
are not touched by opening one.

The theme offers three values, not two: follow the system, light, dark. Two
would force a choice on a reader whose desktop already knows the answer, and the
guideline asks for the third explicitly. The system value is the default,
because a reader who has told their desktop to go dark has told this window too.

The text size is a ratio, not a point size: `100%`, `115%`, `130%`. Every style
in §5 is derived from one base, so one number moves the whole window and no
style can drift out of proportion with the others.

High contrast is a switch, not a fourth theme. It tightens the contrast of
whatever theme is in force rather than replacing it, so a reader who needs it
does not also lose the light-or-dark choice they made. It is applied through the
same tokens as everything else, which is the only way both themes can honour it.

The three live in `App`, not in `egui`, so the state machine can be tested
without a window; the window pushes `App`'s values into the context once per
frame, before the first panel is drawn.

**Keyboard.** Every action the reader can reach with a key, and the window is
fully operable without a mouse — which on `egui` is work rather than a
checkbox, because its accessibility support is thin. Two rules decide
everything:

*Shortcuts resolve after the panels are drawn.* A key a text field has already
taken never reaches the shortcut table, which is what stops a reader typing a
query from turning pages underneath them. Resolution is a pure function in
`shortcuts`, so the whole table is testable without a keyboard.

*Keys and buttons reach the same state.* Paging by key calls the same methods
the toolbar buttons call, so the two cannot drift apart. A test asserts it.

Escape leaves the search box before it closes the search panel: the box takes
focus first, so one key covers both states and the reader is never trapped in a
panel they cannot leave. Shortcuts that act on a page do nothing when no
document is open, which is not a failure worth a message.

**Language.** The two bundled locales are offered in Z1, each written in its own
language. That is the point of an endonym: a reader who cannot read the menu's
current language still has to be able to find theirs in it, so translating the
entries would hide the one they are looking for. The names come from the
catalogue, because they are text and all of the reader's text lives there.
Switching the interface keeps the rendered pages: a page is pixels and a label
is a word, so a translated interface does not make a cached raster wrong.

## 3. Frame flow

`eframe::App::ui` runs ten steps, always in this order. The order is the
specification; a step may not be moved earlier because each one consumes what
the previous produced.

```
1  resolve intent   consume the open request, then dropped files
2  apply state      App mutates: document, page, dpi, layout, error
3  apply appearance push App's theme, text size and contrast into egui
4  sync window      push the document name into the title bar
5  draw Z1          header  — menu, panels, export, settings (reads state)
6  draw Z2          header  — paging, fit, zoom (reads state)
7  draw Z5          search        (only when open; before the canvas)
8  draw Z6          signatures    (only when open; before the canvas)
9  draw Z7          annotations   (only when open; before the canvas)
10 draw Z8          comparison    (only when open; before the canvas)
11 draw Z3          canvas        (the only zone that rasterizes)
12 draw Z4          status bar    (reads state, error wins the left half)
13 resolve keys     what the panels did not take (see below)
```

Step 13 is last on purpose. A shortcut that resolved earlier would fire while a
reader is typing, because at that point no text field has claimed the key; by
running after every panel, the only keys left are the ones nobody wanted.

Step 3 is early on purpose. `egui` resolves a style when a widget is built, so an
appearance pushed after a panel has been drawn would show on the next frame —
one frame of white flash for a reader in dark mode, which reads as a bug rather
than a preference.

Four consequences worth stating, because each one has been got wrong before:

- **Intent is resolved before it is drawn.** Z1 sets the open request; the
  dialog opens on the next frame's step 1, never inside the click handler. A
  modal dialog opened from inside a layout pass corrupts that pass.
- **Z3 is the only rasterizer.** The status bar must never ask for a page. If a
  value requires rendering, it is drawn in the canvas or not at all.
- **State changes land before the frame is painted.** A click in Z1 mutates
  `App`; the canvas in Z3 of the same frame already shows the result. Nothing
  waits a frame, because a frame of lag reads as a dropped click.
- **Appearance is a state setting, not a widget setting.** No zone changes the
  theme, the text size or the contrast; they read `App` and are drawn in whatever
  it says. A panel that styled itself would fight step 3.

Drops are resolved after the open request, so a deliberate menu open is never
raced by a stray drag.

## 4. Rules

- **No literal user-facing text in the window.** Every string comes from
  `locales/<tag>/reader.ftl` through `MessageId`. Symbols that are not language
  (`◀`, `▶`) may be written inline; they carry no meaning a translator can lose.
- **Errors are shown, never swallowed.** A failure reaches the status bar as the
  engine's own message. The window adds no wording of its own, so the reader
  sees one vocabulary.
- **A failed open leaves the previous document open.** Replacing a good document
  with a broken one loses the reader's place for no reason.
- **Every `id_salt` is a literal.** Widget identity must be stable across
  builds; an interpolated salt silently resets scroll position and focus.
- **One texture per page and resolution.** The name carries both so the
  texture and the cache cannot disagree.
- **Blocking work stays off the draw path.** `rfd` and `std::fs` run in step 1
  or in the intent handler, never inside Z3, and rasterizing runs on the worker.
- **Two controls never wear the same word.** A toggle and the panel it opens
  need different labels, or the reader has to guess which one they are on.
- **Colour is never the only cue.** Every coloured state also carries a word or
  a symbol, so the verdict survives a colour-blind reader, a monochrome display
  and a screenshot printed in black.
- **Nothing blinks.** There is no animation that pulses, flashes or loops; the
  only motion is the reader's own, and a page arriving simply appears.
- **A style is a token, not a number.** Colours and point sizes come from the
  tables in §5 through `theme.rs`; a literal colour or `FontId` in a zone is the
  bug that makes one theme look wrong and the other fine.
- **Settings are remembered per reader, not per document.** The theme, text size
  and contrast are unaffected by opening, closing or switching a document.

## 5. Appearance

Three tables decide how the window looks, and they are the only place a colour
or a size is written. The point of putting them in one file is that a fourth
theme, or a fifth text style, is an edit to a table rather than a hunt through
the zones.

### 5.1 Themes

| Token | Light | Dark | What it paints |
| --- | --- | --- | --- |
| `surface` | `#fafafa` | `#242424` | The window behind the panels |
| `panel` | `#ffffff` | `#303030` | A panel or a header |
| `text` | `#1a1a1a` | `#f2f2f2` | Body text |
| `dim` | `#5e5e5e` | `#b8b8b8` | Secondary text: a signer, a count |
| `border` | `#dcdcdc` | `#3f3f3f` | The line between zones |
| `accent` | `#1c71d8` | `#78aeed` | Selection, focus, the current hit |
| `valid` | `#2a7b3c` | `#5fbf78` | A signature that holds |
| `tampered` | `#b42a2a` | `#f0736a` | A signature that does not |
| `unverified` | `#8a6100` | `#e0b341` | A signature nothing can confirm |
| `canvas` | `#eeeeee` | `#1a1a1a` | Behind the page itself |

The light and dark values are different colours rather than one colour at two
opacities: a red that reads as "wrong" on white turns muddy on black, and the
dark column exists so it does not.

High contrast does not add entries to this table. It replaces `text` with pure
black or white, `dim` with the same, and the three verdict colours with a darker
or lighter shade of themselves, so the contrast ratio of every pair rises above
7:1 while the hues stay recognisable. A theme that only changed hues would pass a
checker and fail the reader.

Follow-the-system resolves to one of the two columns and nothing else. There is
no third visual style to keep in step, which is why the setting can offer three
values without three times the work.

### 5.2 Type scale

| Style | Base ratio | Role |
| --- | --- | --- |
| `large-title` | 2.000 | The document name in the title bar |
| `title-1` | 1.600 | A panel title |
| `title-2` | 1.400 | A section inside a panel |
| `title-3` | 1.250 | A group heading |
| `heading` | 1.200 | A heading that is not titled |
| `title-4` | 1.100 | A row heading |
| `body` | 1.000 | Everything read in sentences |
| `caption` | 0.850 | A signer, a time, a count — text under a thing |

The base is 14 points at 100% text size, and every style is that base times its
ratio times the reader's chosen scale. Ratios rather than sizes because the
guideline asks for a scale, and a scale is what makes the text-size setting
possible at all: `130%` multiplies one number and every style follows. The table
is strictly increasing, which a test asserts, because a scale with one ratio out
of order stops being a scale.

`egui` has its own names for six of these. It draws a button from a button style
and a small label from a small one, so those names keep pointing at the ratios
above rather than being removed: the `Button` style gets `body`, `Small` gets
`caption`, `Monospace` gets `body` in the monospace family, and the rest are
this window's own names. Nothing pretends `egui` has a taxonomy it does not.

The ratios are the same in every theme and at every contrast. A style is a role,
not a decoration, and a role that changed size with the theme would move the
layout when the reader switched it.

Two styles the guideline names are deliberately absent. There is no italic —
emphasis is weight or colour, and an oblique CJK glyph is a synthesized slant
rather than a real face. There is no all-caps style for the same reason the
guideline gives: caps are a property of the words, and a translated string is not
this window's to shout.

### 5.3 Spacing and control size

The guideline asks for spacing in steps of six, growing with the distance
between two things, so the window has one base and four multiples of it rather
than four unrelated numbers.

| Token | Step | Where it goes |
| --- | --- | --- |
| `small` | 1 × 6 | A control and its label; two controls in one group |
| `medium` | 2 × 6 | The window edge and its contents; two groups |
| `large` | 3 × 6 | A heading and the thing it heads |
| `extra-large` | 4 × 6 | Zones that belong to different things |

Two rules come with the scale, and they are the ones that make a window look
like one window:

- **A group is separated by space, not by a rule.** The guideline says frames
  and separators are the wrong tool for grouping and that blank space with a
  bold heading is the right one. The window drew thirteen `separator` calls
  before this section existed; it draws none now.
- **A header-bar control has no frame.** Controls in a header bar are drawn in
  the toolbar style — no background, no border — and the frame appears only
  under the pointer, so the header stays quiet while every control stays
  discoverable and keyboard focus still shows.

| Size | Points before the reader's scale |
| --- | --- |
| Control height | 34 |
| Corner radius | 6 |
| Gap between two items in a row | 6 |

All three multiply by the reader's text size. A window whose spacing ignored
the text size would crowd exactly the reader who asked for larger text.

The header bar holds two groups on the start edge — which document is open, and
what is shown of it — and four menus on the end edge: the language, the
comparison, the export and the settings. That is where the guideline puts menus.
What is *not* there is one button per option: a language is a menu rather than
three buttons, and an export is a menu rather than three buttons, because the
guideline asks for a small number of controls so that the bar can be dragged and
the window can be narrowed without the controls colliding.

### 5.4 Using them

`theme.rs` is the one module that knows these numbers. A zone asks it for a
colour (`theme::colour(ctx, sharp, Token::Dim)`) or for a size
(`Style::Title1.points(scale)`), and the reader's scale is applied by rebuilding
the `egui` style table once per frame in step 3.

Where a style and an `egui` name stand for the same role the two are the same
entry, so a button or a tooltip cannot drift out of the scale. Where they do
not, the zone asks the table directly.

Text is drawn with a named style, never with `.size(...)`. A point size in a zone
is a style that has escaped the scale, and it is the reason a window ends up with
six sizes that are nearly the same. `theme::apply` is the only exception and it
is the scale itself.

One `egui` behaviour is worth recording because it cost an afternoon: calling
`Context::set_theme` from inside a frame changes which style is active while the
style already handed out is still in use, and the pass then draws with one
theme's colours and the other's sizes. The window therefore resolves the scheme
itself and dresses that one table, and never moves the global preference.

## 6. Testing

The window is tested twice over, and the split matters.

**`tests/reader.rs` proves the state machine**, calling `App` methods directly.
Fast, and it can reach states a click cannot.

**`tests/window.rs` proves the layout draws that state**, building a real
`egui` widget tree through `egui_kittest` and running frames with no display
and no X server. It clicks the actual toolbar buttons and reads labels out of
the catalogue, so a string that loses its entry, a control that stops wiring to
`App`, or a raster too large for the driver all fail in CI rather than in front
of a reader. Labels are never written as literals in this file; if a test needs
to find a control, it asks the catalogue what that control says.

A change to the window that cannot be expressed as a change to `App` needs a new
method there first. If it cannot be, the feature does not belong in this reader.

The appearance is tested the same way it is written. A test asserts that every
theme column defines every token, that high contrast raises the contrast of each
text-on-panel pair rather than only changing its hue, and that the type scale is
monotonic — a `title-1` is never smaller than a `title-2`. A theme table is a
small thing to test and a large thing to get wrong quietly.

## 7. Adding a zone

A new zone is admitted only if it cannot live in an existing one, and it must
come with: its position in the stack above, its step in the frame flow, its
disabled-state rule, its strings in both catalogues, its tooltips, and a test for
whatever `App` method it drives. A zone that paints a colour or a size of its own
is not admitted: it names a token or it does not ship.
