# 07 — UI & Viewer Architecture

Source: `org.igv.ui`, `org.igv.ui.panel`, `org.igv.circview`, `org.igv.ui.svg`.
Concepts are emphasized; Swing plumbing is noted only where it encodes a design idea.

**TL;DR** IGV's UI is: one `IGV` singleton owning a `Session` and window scaffolding; a `MainPanel` that lays out a vertical stack of `TrackPanel`s (each = name header + attribute header + `DataPanelContainer`); a `DataPanelContainer` that creates one `DataPanel` per `ReferenceFrame`; and a single mutable viewport object (`ReferenceFrame`: origin + scale in bp/pixel, derived zoom) that all rendering is parameterized by. Navigation (search/zoom/pan/gene lists) ultimately just mutates `ReferenceFrame`s and posts `ViewChange` events on a global `IGVEventBus`.

**Coordinate conventions (global):** IGV stores everything internal in **0-based, end-exclusive** ranges (`org.igv.feature.Range`: `chr, start, end`). Locus _strings_ are UCSC-style **1-based inclusive** (`chr1:100-200` ⇒ start=99, end=200, no +1 on the end). Display strings re-add +1 to start only (`Locus.getFormattedLocusString`). Chromosome lengths are in bp; the whole-genome view uses a pseudo-chromosome `"chrAll"` (`Globals.CHR_ALL`) whose coordinate space is the concatenated sum of chromosome lengths.

## Panel hierarchy

```
IGVContentPane (org.igv.ui.IGVContentPane)
└── MainPanel (org.igv.ui.panel.MainPanel)          // horizontal layout, Paintable
    ├── HeaderPanelContainer (NameHeaderPanel / AttributeHeaderPanel / HeaderSelectAllPanel)
    ├── TrackPanelScrollPane  ← one per TrackPanel, wrapped in a JScrollPane (vertical scroll)
    │   └── TrackPanel (org.igv.ui.panel.TrackPanel)   // one panel group; Scrollable + Transferable (DnD)
    │       ├── TrackNamePanel      // left: track names, drag handles, reorder
    │       ├── AttributePanel      // left, below names: per-track attribute rows
    │       └── DataPanelContainer (org.igv.ui.panel.DataPanelContainer)
    │           └── DataPanel  × N   // one per ReferenceFrame; owns frame + DataPanelPainter
    └── TrackPanelDivider           // draggable boundary between panels
```

- **Tracks map to panels**: each `Track` has an `order` property; `MainPanel.addTrackPanel(Track)` inserts a new `TrackPanel` at the position matching the track's order (ordered list of `TrackPanelScrollPane`s). So **the unit of panel is the "panel group"** — a single track (possibly a `TrackGroup` of data tracks stacked inside it). `TrackPanel.getTracks()`, `containsTrack`, `addTrack(s)`, `removeTracks`, `sortTracksByAttributes/Position`, `sortByRegionsScore` implement group behavior. `clearTracks` + `removeEmptyPanels` preference control pruning.
- **`DataPanel`** (`org.igv.ui.panel.DataPanel`) — binds one `ReferenceFrame` + a `DataPanelPainter`; implements `Paintable` and `IGVEventObserver` (subscribes to `ViewChange` for repaint). Contains mouse/zoom/pan tools (`DataPanelTool`, `PanTool`, `RegionOfInterestTool`) and renders `RegionOfInterest` overlays.
- **`DataPanelPainter.paint(track, RenderContext)`** — the track draw entry; handles the special "expanded insertion" layout (extra screen pixels injected for an insertion) before delegating per-frame painting.
- **`RenderContext`** (`org.igv.track.RenderContext`) — carries `Graphics2D`, the `ReferenceFrame`, clip `Rectangle`, and a graphics cache; tracks draw purely from context ⇒ rendering is already separable from Swing.
- **`FrameManager`** (`org.igv.ui.panel.FrameManager`) — static registry of the current `List<ReferenceFrame>`; `frames.size() > 1` = "gene list mode". `DataPanelContainer.createDataPanels()` builds one `DataPanel` per frame (shrinking `hgap` when >10 frames); `PanelLayoutManager`-style horizontal slicing shares the container width among frames. `getMinimumScale()` returns min scale across frames so all frames stay in sync when stacking.

## ReferenceFrame — viewport state

`org.igv.ui.panel.ReferenceFrame` (plain state object + event posts; copy constructor exists).

| Field                          | Meaning                                                                                                                                                   |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `double origin`                | left edge of viewport in bp (0-based; may go slightly negative for soft-clip overhang)                                                                    |
| `double scale`                 | bp per **virtual** pixel (see below)                                                                                                                      |
| `String chrName`               | current chromosome, or `"chrAll"` for whole-genome view                                                                                                   |
| `int zoom`                     | `nTiles = 2^zoom`; zoom −1 conceptually = whole genome; clamped to `[minZoom, maxZoom]`, `maxZoom` computed from chr length, `minBP = 40` floor on window |
| `int pixelX, widthInPixels`    | frame's x-offset and width in real screen pixels                                                                                                          |
| `static int binsPerTile = 700` | nominal tile width in pixels — the virtual-pixel device                                                                                                   |
| `Locus initialLocus`           | pending locus from navigation; consumed by `computeLocationScale()` once pixel width is known                                                             |

**Viewport math:**

- `getScale() = chromosomeLength / max(nTiles·binsPerTile, widthInPixels)` — i.e. zoom defines a _virtual_ canvas (`2^zoom × 700 px`) of the whole chromosome; at zoom 0 the virtual canvas equals ~screen width. Pan/zoom-in-place just re-derives `origin`/`scale` around a fixed center (`doSetZoomCenter`, `doIncrementZoom`, `getGenomeCenterPosition`).
- `chromosomePosition(px) = origin + scale·px`; `screenPosition(bp) = (bp − origin)/scale` (`getChromosomePosition`, `getScreenPosition`, with adjustment for expanded insertions).
- `getEnd() = chromosomePosition(widthInPixels)`; window length in bp = `widthInPixels × scale`.
- `calculateZoom(start, end) = round(log2(chrLength/windowLength × widthInPixels/binsPerTile))`.
- Mutations (`setOrigin`, `changeZoom`, `changeChromosome`, `jumpTo(chr,start,end)` / `jumpTo(Locus)`, `dragStopped`) clamp to chromosome bounds then **post `ViewChange.LocusChangeResult` / `ChromosomeChangeResult` to `IGVEventBus`** — the event bus is the repaint/history propagation mechanism; components observe events rather than call each other. History is recorded via `recordHistory()` into the session.
- `stateHash()` — cheap fingerprint (origin/scale/chr/zoom bits) used to decide whether re-render is needed.

**Navigation model:** pan = `shiftOriginPixels(deltaPx)` (± scale·delta); zoom preserves center; drag end snaps origin to integer bp. Whole-genome view = `chrName="chrAll"`, origin 0, scale = chrAll length / virtual pixels.

## Locus strings & search

Two parsers with different strictness:

- **`org.igv.feature.Locus`** (extends `Range`) — strict: requires both `:` and `-`; `parseLocusString` splits on `:`, strips commas, `start = max(0, parsedStart − 1)` (**1-based input → 0-based inclusive**), `end` used as-is (**inclusive end kept as exclusive end**, i.e. no adjustment). Single position `chr:100` ⇒ `end = start+1`. `toString()` prints 0-based (`chr:start-end`); `getFormattedLocusString` prints 1-based.
- **`org.igv.ui.action.SearchCommand`** — the general search bar path, used via `FrameManager.getLocus(searchString[, flankingRegion])`. Grammar accepted per token:
  - Locus forms: `chr`, `chr:pos`, `chr:pos1-pos2`, whitespace forms `chr pos` / `chr pos1 pos2`, `*`/`all` ⇒ whole-genome (`CHR_ALL, 0, Integer.MAX_VALUE`).
  - `ResultType`: `FEATURE`, `LOCUS`, `ERROR`, `LIFTOVER`.
  - Non-locus resolution order: HGVS notation → MANE transcript → genome `FeatureDB` name match (longest feature preferred, canonical-chr matches preferred) → optional webservice lookup → protein/nucleotide mutation syntax (`GENE:A123B`, `GENE:123G>T`).
  - `getStartEnd`: input positions are 1-based (`−1` applied to start); a window < 10 bp (or single position) is widened to ±20 bp around the center.
  - Flanking region (preference, bp or % when negative) is added only for non-`LOCUS` results (gene/feature hits), never for explicit coordinate loci.
  - Multiple space-delimited tokens yield multiple results, each shown; a single valid result exits gene-list mode (`Session.setCurrentGeneList(null)`).

## Region of interest / region navigator

- **`org.igv.feature.RegionOfInterest`** — `{chr, start, end (0-based chromosomal), description, selected, static fg/bg colors}`. Stored per-attribute-key in `Session.regionsOfInterest: Map<String, Collection<RegionOfInterest>>` with an observable wrapper for UI updates.
- UI: `org.igv.ui.panel.RegionOfInterestPanel` (sidebar list), `RegionNavigatorDialog` (table + go-to), `RegionOfInterestTool` (drag-to-define on a `DataPanel`), `RegionMenu`; rendered by `DataPanel.drawRegion`. Regions also drive track sorting (`TrackPanel.sortByRegionsScore` with a `RegionScoreType`).

## Gene lists (`org.igv.lists`)

- **`GeneList`** — `{name, description, group, List<String> loci}` where each locus is a raw locus/gene string (parsed lazily via `FrameManager.getLocus`). Managed by `GeneListManager` (persisted XML user lists); `GeneListGroup` groups them.
- **Frame model:** `FrameManager.resetFrames(GeneList)` clears frames; 1 locus ⇒ jump the default frame; N loci ⇒ create one `ReferenceFrame` per locus (`addNewFrame`, named by the locus string), side-by-side `DataPanel`s; frames sorted by `FRAME_COMPARATOR`. `isGeneListMode()` = `frames.size() > 1`. `IGV.setGeneList` wires this plus session state (`Session.currentGeneList`, `geneListMode`).

## Circview (`org.igv.circview`) — brief

A standalone circular genome view (port of a JBrowse-style `circularView.js`), not integrated into the main `TrackPanel` stack.

- **Model:** `Chord {uniqueId, refName, start, end (0-based), Mate{refName,start,end}, color?}` — one arc; built from `BedPE`, read mates/supplementary alignments, or `Variant`s. `ChordSet {name, trackName, chords, color}`; `ChordSetManager` keeps both a flat `List<ChordSet>` and grouped `List<Track>` (group-by-track flag selects which to render). `Assembly` = named chromosome collection.
- **Layout:** `GenomeArcLayout` maps chromosome lengths to angular arcs around a circle (start at −π/2, clockwise, uniform gap angle); `pointAt(angle, radius)` for endpoints. Chord arcs are quadratic/bezier curves; `CircularView` retains `RenderedChord` geometry for hit-testing/click listeners.

## Image / SVG capture

- `Paintable` interface (`paintOffscreen(Graphics2D, Rectangle, batch)`, `getSnapshotHeight(batch)`) is implemented by `MainPanel`, `DataPanelContainer`, `DataPanel`, `TrackPanel`, etc. — the same draw path as on-screen painting, so snapshots are "render the component offscreen with a bigger canvas".
- `IGV.createSnapshot`/`createSnapshotNonInteractive(Component, File, batch)` → `org.igv.ui.util.SnapshotUtilities.doComponentSnapshot`: dispatch on extension via `ImageFileTypes` (PNG and SVG only; EPS/JPEG rejected). PNG: `BufferedImage` + standard `Graphics2D`. SVG: Batik `SVGGraphics2D` over an empty DOM document, then `stream(out)`. Note `org.igv.ui.svg.SVGGraphics` (a hand-rolled `Graphics2D→SVG` emitter) exists but is marked **experimental/unused**.
- `batch` mode increases snapshot height to include all content (not just visible) — used by the batch/snapshot command tool.

## The IGV singleton

`org.igv.ui.IGV` — eager-checked singleton (`createInstance` throws if re-created; `getInstance` throws if absent). Owns:

- **Window scaffolding:** `mainFrame`, `rootPane`, `IGVContentPane`, `IGVMenuBar`, glass pane, custom cursors, status-bar messages (`setStatusBarMessage*`), `MainPanel` accessor.
- **`Session session`** (`org.igv.session.Session`) — the _serializable_ user state: `path`, `version`, `referenceFrame` (default frame), `preferences`, `colorScales: Map<DataType, ContinuousColorScale>`, `sampleFilter`, `history`, `regionsOfInterest` (+ observable), `currentGeneList`, `geneListMode`, `hiddenAttributes`, `locus` string, `removeEmptyPanels`. Autosaved on a timer.
- **Track/panel aggregation:** no owned track list — `getAllTracks()` folds `getTrackPanels()` (delegated to `MainPanel`). Also `getFeatureTracks/getDataTracks/getAlignmentTracks`, `getLoadedTypes`, `getDataResourceLocators` (via tracks). `addTrackPanel` delegates to `MainPanel` with order-based insertion.
- **Repaint orchestration:** `isLoading`, `repaintPending`, `repaintLock` — coalesces async reloads into single repaints; `checkPanelLayouts`.
- **Misc:** recent sessions/URLs (`RecentFileSet`, `RecentUrlsSet`), event subscriptions (`subscribeToEvents`), ruler toggle.

`FrameManager` is a second, static mini-singleton for the frame list — notably _not_ persisted in the session beyond the default frame + locus string; gene-list frames are rebuilt from the gene list on session load.

## Mab notes

Transferable concepts:

- **Viewport = {origin: f64 bp, scale: f64 bp/px, width: px}**; zoom is derived, not primary (`zoom = log2(len·px/(window·tile_px))`). Treat the 700-px virtual tile as an implementation detail; a Rust port can store window length in bp directly and compute zoom on demand. Clamps: min window 40 bp, origin ≥ 0 (allow negative only for soft-clip padding).
- **Split view is N viewports sharing one panel column** — a `Vec<Viewport>` plus per-frame `DataPanel`s; "gene list mode" falls out naturally from `frames.len() > 1`. A gene list is just a named `Vec<String>` of locus strings; rebuild frames from it (state reconstruction, not serialization).
- **Search pipeline layering:** strict locus regex first, then feature-name DB (prefer longest feature, canonical chr), then optional remote lookup, then HGVS/mutation syntax; add flanking only for feature hits; widen tiny windows to ±20 bp. Keep input 1-based inclusive → internal 0-based half-open at exactly one boundary.
- **Decouple rendering from the toolkit:** IGV's `Paintable` + `RenderContext(frame, clip, graphics)` pattern maps directly to a Rust `fn paint(&Track, &Viewport, &mut Canvas, clip: Rect)`; snapshot/export is then just a different `Canvas` backend (PNG raster / SVG writer) over the same code, with a "full content height" flag.
- **Event-bus navigation:** all view mutations (pan/zoom/chr change/jump) funnel through one event type (`ViewChange`) carrying frame + resulting range; observers (repaint, history, status bar) subscribe. In Rust an owned `Vec<Observer>` or a message enum on a channel is fine; the key is that view state has a single writer (the frame) and derived readers.
- **Session vs frames:** persist the _default_ frame + locus string + gene list; re-derive split frames. Regions of interest are simple `{chr, start, end, description}` records keyed by attribute.

Swing-specific (do not port): `TrackPanel` as `Transferable` for DnD reordering (replace with an explicit reorder command), `JScrollPane`/`Scrollable` height negotiation, cursors/glass panes, the static `FrameManager` (use owned state), `IGV` god-singleton (in Rust prefer a `Viewer` struct owning `Session` + panels, passed by context).
