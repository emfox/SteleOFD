# Reader UI Layout and Frame Flow

This is the contract every reader surface follows. It exists because the reader
has three layers that can each be written in any order, and without a rule the
window turns into one long function whose behaviour nobody can test.

The rule the rest of this document serves: **the window draws, the state
decides.** `main.rs` may only read state and ask `App` to change it; `App` may
never know a widget exists. Everything testable therefore lives in `app.rs` and
`viewer.rs`, and the tests never open a window.

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
│ Z1  menu bar      fixed      open · close · layout      │  Panel::top("menu")
├────────────────────────────────────────────────────────┤
│ Z2  toolbar       fixed      paging · fit · zoom        │  Panel::top("toolbar")
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
```


**Z1 menu bar.** Document-level actions only: open, close, and the layout
choice. Anything that acts on the current page belongs in Z2. The close button
is disabled when nothing is open, so the bar never has to explain itself.

**Z2 toolbar.** Actions on what is on screen: previous, next, fit width, fit
page, zoom. Disabled controls rather than hidden ones, so the bar does not
reflow as the document changes.

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
Three verdicts, not two: `Valid`, `Tampered`, and `Unverified`. The engine can
only conclude that a signature is good or bad, and reports a signature it has no
certificate for as an error; flattening that into "tampered" tells the reader
their document was altered when nothing says it was. Unverified is the honest
answer, and it is shown in amber so it is not mistaken for either verdict.
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

## 3. Frame flow

`eframe::App::ui` runs nine steps, always in this order. The order is the
specification; a step may not be moved earlier because each one consumes what
the previous produced.

```
1  resolve intent   consume the open request, then dropped files
2  apply state      App mutates: document, page, dpi, layout, error
3  sync window      push the document name into the title bar
4  draw Z1          menu bar      (reads state, writes intent)
5  draw Z2          toolbar       (reads state, writes intent)
6  draw Z5          search        (only when open; before the canvas)
7  draw Z6          signatures    (only when open; before the canvas)
8  draw Z3          canvas        (the only zone that rasterizes)
9  draw Z4          status bar    (reads state, error wins the left half)
```

Three consequences worth stating, because each one has been got wrong before:

- **Intent is resolved before it is drawn.** Z1 sets the open request; the
  dialog opens on the next frame's step 1, never inside the click handler. A
  modal dialog opened from inside a layout pass corrupts that pass.
- **Z3 is the only rasterizer.** The status bar must never ask for a page. If a
  value requires rendering, it is drawn in the canvas or not at all.
- **State changes land before the frame is painted.** A click in Z1 mutates
  `App`; the canvas in Z3 of the same frame already shows the result. Nothing
  waits a frame, because a frame of lag reads as a dropped click.

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

## 5. Testing

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

## 6. Adding a zone

A new zone is admitted only if it cannot live in an existing one, and it must
come with: its position in the stack above, its step in the frame flow, its
disabled-state rule, its strings in both catalogues, and a test for whatever
`App` method it drives.
