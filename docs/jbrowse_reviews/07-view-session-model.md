# 07 — Views, Navigation, and the Session Model

> JBrowse 2 v4.3.0 (commit 83ac4507cf). All paths repo-relative. Coordinates:
> genomic intervals are **0-based half-open** `[start, end)` (BED/BAM-style)
> everywhere in state and across the worker boundary; locStrings and tick
> labels shown to users are **1-based inclusive**. Pixel coordinates are
> CSS px in the view's coordinate frame; `offsetPx` is a position in the
> _linearized_ (concatenated displayedRegions) pixel space.

## TL;DR

- The LinearGenomeView (LGV) state is tiny and transfers directly to Rust:
  a `displayedRegions: Region[]` list (the "layout"), plus a viewport
  expressed as **a window in linearized bp** (`windowStartBp`,
  `windowWidthBp`) — not pixels. `bpPerPx` and `offsetPx` are _derived_
  (`bpPerPx = windowWidthBp / width`, `offsetPx = windowStartBp / bpPerPx`).
- Everything else on the view is derived geometry: staticBlocks (render
  chunks of ~800 CSS px), dynamicBlocks (exact visible content),
  visibleRegions (the fetch window handed to workers).
- Sessions are plain MST snapshots: `{ id, name, views[], widgets,
  sessionTracks, trackConfigDeltas, connections, ... }`. Sharing encodes
  the same snapshot three ways (`share-<id>` server + password,
  `encoded-<base64>`, `json-<json>`); autosave is a debounced IndexedDB
  write.

---

## 1. LinearGenomeView — the viewport model

File: `plugins/linear-genome-view/src/LinearGenomeView/model.ts`
(`stateModelFactory`, ~3200 lines), composed from
`packages/core/src/pluggableElementTypes/models/BaseViewModel.ts`
`BaseViewModel` + `HighlightsMixin`.

### BaseViewModel (all views inherit)

| field         | type        | notes                                      |
| ------------- | ----------- | ------------------------------------------ |
| `id`          | `ElementId` | MST id (or uuid)                           |
| `displayName` | `string?`   | header label; falls back to assembly names |
| `minimized`   | `boolean`   | collapsed to header bar                    |

Volatile: `width` (default 800; real width measured at mount).

### Persisted properties (`#property`, survive session save)

| field                                                    | type                                    | role                                                                                                                                                               |
| -------------------------------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `type`                                                   | `'LinearGenomeView'`                    | discriminator                                                                                                                                                      |
| `windowStartBp`                                          | `number` (default 0)                    | **Left edge of viewport in linearized bp** — concatenated displayedRegions space. May be negative (scrolled past left end). No inter-region padding in this space. |
| `windowWidthBp`                                          | `number` (default 0)                    | viewport width in bp. `0` = "not measured yet"; first `setWidth(n)` fills it (`windowWidthBp = (legacyBpPerPx                                                      |
| `legacyBpPerPx`                                          | `number` (default 0)                    | migration-only: carries a pre-window snapshot's `bpPerPx` to the first measure, then cleared (see preProcessSnapshot below).                                       |
| `displayedRegions`                                       | `frozen Region[]`                       | the layout — see §2                                                                                                                                                |
| `tracks`                                                 | `array(track stateModel)`               | instantiated track models (their display state is persisted inside each)                                                                                           |
| `hideHeader`, `hideHeaderOverview`, `hideNoTracksActive` | `boolean`                               | chrome switches                                                                                                                                                    |
| `trackSelectorType`                                      | `'hierarchical'`                        | vestigial, kept for old snapshots                                                                                                                                  |
| `showCenterLine`                                         | `boolean`                               | localStorage-backed; never persisted (postProcessSnapshot strips it)                                                                                               |
| `showCytobands`                                          | `boolean`                               | localStorage default true; persisted only when non-default                                                                                                         |
| `trackLabels`                                            | `''\|'overlapping'\|'offset'\|'hidden'` | localStorage-backed                                                                                                                                                |
| `showGridlines`                                          | `boolean`                               |                                                                                                                                                                    |
| `labelsVisible`                                          | `boolean`                               | highlight/bookmark chip labels                                                                                                                                     |
| `colorByCDS`                                             | `boolean`                               | reading-frame coloring                                                                                                                                             |
| `showAminoAcids`                                         | `boolean`                               | translated codons at base zoom                                                                                                                                     |
| `showTrackOutlines`                                      | `boolean`                               |                                                                                                                                                                    |
| `scalebarOnly`                                           | `boolean`                               | render header+scalebar only                                                                                                                                        |
| `init`                                                   | `frozen InitState?`                     | transient launch spec (see §7); stripped on save once displayedRegions exist                                                                                       |

From `HighlightsMixin` (`packages/core/src/pluggableElementTypes/models/HighlightsMixin.ts`):

- `highlight: frozen HighlightType[]` — translucent bands `{assemblyName,
  refName, start, end}`; note `start/end` here are **0-based half-open**.
- `showHighlightChips: boolean`.
  Actions: `addToHighlights/setHighlight/removeHighlight/updateHighlight`.

`HighlightType` = `{ assemblyName, refName, start, end }` (0-based half-open).

### Volatile (not persisted)

`volatileWidth` (measured width px), `minimumBlockWidth` (3), `coarseDynamicBlocks`
/`coarseTotalBp`/`coarseBpPerPx` (500 ms-debounced copies of the visible blocks,
used by autoscale scans and location box), `leftOffset`/`rightOffset`
(`BpOffset | undefined` — the rubberband selection), `draggingTrackId`,
`volatileError`, `volatileGuides` (per-display vertical guides).

`BpOffset` = `{ refName, start, end, offset, index, oob? }` where `start/end`
are the displayed region's bounds (0-based), `offset` is bp offset from the
region's left edge, `index` the displayedRegions index.

### Derived getters — the viewport math

```
bpPerPx   = windowWidthBp / volatileWidth        // 0 before first measure
offsetPx  = windowStartBp / bpPerPx
totalBp   = Σ (r.end − r.start) over displayedRegions          // 0-based widths
displayedRegionsTotalPx = totalBp / bpPerPx
```

Zoom limits (`model.ts`):

- `minBpPerPx = MIN_BP_PER_PX = 1/50` (`consts.ts`) — i.e. **50 px per bp**
  maximum zoom-in.
- `fitBpPerPx = max(MIN_BP_PER_PX, totalBp / (width * 0.9))` —
  `SHOW_ALL_REGIONS_FILL = 0.9` leaves a 10% margin at max zoom-out.
- `maxBpPerPx = max(fitBpPerPx, sharedFitBpPerPx)` — a containing comparative
  view (sameScale mode) may raise the zoom-out ceiling.
- Scroll bounds: `minOffset = −width + 30`, `maxOffset = displayedRegionsTotalPx − 10`
  (`MIN_OFFSET_PADDING_PX = 30`, `MAX_OFFSET_PADDING_PX = 10`) — keeps some
  content pinned to an edge.

Coordinate conversions (`packages/core/src/util/Base1DUtils.ts`):

- `pxToBp(self, px)` — px is viewport-relative; walks displayedRegions
  accumulating bp and returns `{ refName, start, end, offset (bp, float),
  index, coord (1-based, floor+1), coord0 (0-based), oob }`. Errors on empty
  displayedRegions.
- `bpToPx({refName, coord, displayedRegionIndex?}, self)` — takes a **0-based**
  coord; returns `{ index, offsetPx: round((cumulativeBp + coord − r.start)/bpPerPx) }`.
- `bpToLinearBp` / `offsetBpToPx` — float-precise variants used by animations
  (avoid per-frame 1-bp loss from the floor/round round trip).
- `moveTo(start?: BpOffset, end?: BpOffset)` — places the viewport so the two
  BpOffsets are its edges (clamped).
- `computeMoveToLayout`, `createOverviewLayout`, `getLayoutHighlightCoords`
  — same math parameterized as a `ViewLayout = { displayedRegions, bpPerPx,
  offsetPx, width, minimumBlockWidth }`, so the header overview scalebar and
  the rubberband reuse it.

**Not present:** the classic JBrowse 1 "inter-region padding" between
displayedRegions does not exist in this version — regions are laid out
contiguously; `calculateStaticBlocks` emits boundary padding blocks only
before the first and after the last region, and the LGV draws 3px "seam"
spans at each region's right edge (`paddingSpans` getter). There is no
`hideRegions`/`softClip` state on the model; region elision at coarse zoom is
a _derived_ per-block property (`ElidedBlock`), not stored state. Whole-genome
mode is simply "displayedRegions == all assembly regions" (`showAllRegionsInAssembly`,
and `showsWholeChromosome` checks a single displayed region equals the
assembly's region for that refName, gating cytobands).

### Blocks (the render geometry)

`packages/core/src/util/calculateStaticBlocks.ts` / `calculateDynamicBlocks.ts`:

- **staticBlocks**: chop displayedRegions into content blocks of
  `blockSizeBp = ceil(800 * bpPerPx)` plus `ElidedBlock` (regions narrower than
  `minimumBlockWidth`) and boundary padding blocks. Blocks are the unit tracks
  cache/cover (block `key` encodes assembly/refName/start/end).
- **dynamicBlocks**: exact visible content blocks (no padding, optionally no
  elision — dotplot axes call with `padding=false, elision=false`).
- `staticBlocksTranslateX = staticBlocks.offsetPx − offsetPx` — the one
  subtraction that must happen in f64 before CSS (f32 transforms at whole-
  genome px offsets destroy precision). Relevant to any Rust renderer doing
  pan on f32 GPU: offset per-block/frame in f64, subtract early.
- `visibleRegions` — content blocks as `{refName, start, end, assemblyName,
  reversed, displayedRegionIndex, screenStartPx, screenEndPx}`; this is the
  canonical "fetch window". `bufferedVisibleRegions` expands ±0.5 screen-widths
  in bp, clamped, integer-rounded.
- `visibleWholeBaseRegions` — the same but whole bases (for locStrings/bookmarks).
- `coarseDynamicBlocks` — debounced 500 ms copies; `settleCoarseBlocks()`
  flushes them on discrete jumps (every navigation action calls it).

## 2. displayedRegions — the layout / "RegionSet"

- Type: `Region` = `{ refName, start, end, assemblyName, reversed }`
  (`packages/core/src/util/types`). **0-based, start < end always** even for a
  reversed region — `reversed` only means the axis draws right-to-left.
- Assembled by `showRegions(regions, location?)` /
  `showAllRegionsInAssembly(assemblyName)` (copies `assemblyManager.get(name).regions`)
  or declaratively via `init.displayedRegionNames` (whole chromosomes, resolved
  through assembly aliases, in given order).
- Reversal: `horizontallyFlip()` reverses the list order and flips each
  region's `reversed`; scroll adjusts to stay on the same content.
- One view may mix regions from multiple assemblies (synteny rows); a region
  list may repeat a refName (collapsed introns, read-vs-ref axes).
- There is no first-class `RegionSet` model; the assembly's `regions` array
  (`packages/core/src/assemblyManager/assembly.ts`) is the source, and the view
  holds a frozen copy. Canonical refName resolution happens **before** regions
  enter the model (`generateLocations` uses `assembly.getCanonicalRefName2`).

## 3. Rubberband selection & overview

- Selection state is the volatile pair `view.leftOffset` / `view.rightOffset`
  (`BpOffset`), set by `setOffsets(left?, right?)`. `MultiLevelRubberband`
  (`plugins/linear-genome-view/src/MultiLevelRubberband/`, `useRangeSelect.ts`)
  is a component over any `{ views: LinearGenomeViewModel[],
  rubberBandMenuItems(): MenuItem[] }` (`types.ts`), so LinearComparativeView
  and BreakpointSplitView share it; it computes drag → BpOffset per level and
  offers the rubberband menu (zoom to region, get sequence, highlights, launch
  points via `rubberBandLaunchMenuItems()` extension point).
- `getSelectedRegions(leftOffset, rightOffset)` → `Region[]`: builds a temp
  layout via `computeMoveToLayout`, clamps, then `wholeBaseRegions(dynamicBlocks)`
  — i.e. the selection is converted to **whole-base 0-based half-open regions**.
- Overview: `overviewLayout` = `createOverviewLayout({displayedRegions,
  width − cytobandOffset, minimumBlockWidth})` laid out at width-scale; the
  "you are here" rectangle is `overviewRegionPxSpan`. Cytobands render only
  when the view shows a whole chromosome (`canShowCytobands`).

## 4. Navigation actions

Pixel-facing (kept for gestures and old snapshots):

- `scrollTo(offsetPx)` — clamps to `[minOffset, maxOffset]`, writes
  `windowStartBp = offsetPx * bpPerPx`.
- `zoomTo(bpPerPx, offset = width/2)` — clamps zoom; anchors the base under
  `offset` px exactly in bp arithmetic (no pixel round trip).
- `horizontalScroll(distance)`, `slide(viewWidths)` (spring-animated pan).
- `setNewView(bpPerPx, offsetPx)` — legacy pair.

bp-facing (the real API — survives window resizes):

- `scrollToBp(startBp)`, `setWindow(windowWidthBp, windowStartBp)` /
  `setWindowFrame` (no coarse-block settle; for per-frame writers).
- `showAllRegions()` (to maxBpPerPx), `fitAllRegions()` (edge-to-edge, distinct!),
  `clearView()` (empty regions + tracks).
- `navTo(NavLocation, grow?)` / `navToMultiple` — requires the location to be
  contained in an existing displayedRegion; `grow` pads the interval by a
  fraction, clamped to the containing region. Does **not** change regions.
- `navToLocString(input, assemblyName?, grow?)` — full dispatch via
  `searchUtils.handleSelectedRegion` (see below).
- `navToLocations(regions, ...)` / `navToLocation` — will **replace
  displayedRegions** if needed (`showRegions` = `setDisplayedRegions` + nav/fit,
  one transaction).
- `centerAt(coord, refName)`, animated twins `flyTo(centerBp, windowWidthBp)`
  (Van Wijk-style arc, `flyTo.ts` `planFlight`) and `flyToCenter`; all flights
  yield to any other view movement and land exactly where `setWindow` would.
- Every discrete navigation ends with `settleCoarseBlocks()`.
- Undo/redo of region changes is implemented at call sites
  (`showRegionsWithUndo.ts`), not as model history — the model keeps no nav
  history.

### searchUtils (`plugins/linear-genome-view/src/searchUtils.ts`)

`handleSelectedRegion({input, model, assemblyName, grow})` dispatch order:

1. all whitespace-separated tokens are valid refNames → `navToLocations(parseLocStrings(...))`;
2. input contains `*` and assembly loaded → glob refNames (`matchRefNames`,
   `MAX_GLOB_REGIONS` cap, refuses with a notification rather than truncating);
3. otherwise text search via session `textSearchManager` (`fetchResults`),
   preferring exact hits; >1 result opens a picker dialog
   (`showSearchResults` → `setSearchResults`), 1 result navigates
   (`navToOption`), 0 falls back to locstring parse, reframing unknown-ref
   errors as "No results found" for bare tokens.

Locstring grammar (`packages/core/util` `parseLocString`): `chr1:1,000-2,000`,
`chr1:1-100 chr2:1-100` (space-separated multi-region), `chr 1 100` triplets;
**1-based inclusive** at the string boundary, converted to 0-based half-open
in the model. Autocomplete budget: `MAX_REFNAME_HITS = 10` refName suggestions
ahead of text-search hits.

## 5. Other view models (fields only)

All compose `BaseViewModel` (`id`, `displayName`, `minimized`) and carry an
`init: frozen` launch blob that is stripped once applied.

### CircularView — `plugins/circular-view/src/CircularView/model.ts`

- `offsetRadians` (the pan, default ~π), `bpPerPx` (zoom; capped via
  `minimumRadiusPx`), `autoFit` (cleared on manual zoom/pan so the persisted
  view survives resize), `height`, `displayedRegions: frozen Region[]`,
  `minimumRadiusPx`, `spacingPx`, `paddingPx`, `minVisibleWidth` (elide thin
  arcs), `hideVerticalResizeHandle`, `hideTrackSelectorButton`,
  `disableImportForm`, `tracks[]`, vestigial `trackSelectorType`.
- Volatile: `panX/panY`, `volatileWidth`, `volatileError`.
- Chord tracks store their own `chords` data (fetched once for the whole view).

### DotplotView — `plugins/dotplot-view/src/DotplotView/model.ts`

- `assemblyNames: string[]` (h axis first), `height` (600), `hview`/`vview`
  (one `Dotplot1DView` each — `plugins/dotplot-view/src/DotplotView/1dview.ts`,
  extends `packages/core/src/util/Base1DViewModel.ts` `Base1DView`:
  `{displayedRegions, bpPerPx, offsetPx, minimumBlockWidth, volatileWidth}`),
  `drawCigar`, `showGridlines`, `lockAspectRatio` (keeps hview/vview at the
  same bpPerPx), `lodMode` ('auto'|'fine'|'coarse'), `lineWidth` (2.5),
  `alpha` (1), `minAlignmentLength` (0), `minIdentity` (0), `tracks[]`.
- Syntenic blocks are fetched by the single `DotplotDisplay` track from a
  synteny (PIF/PAF) adapter — the view itself stores only axes; highlight drag
  → `HighlightType` per axis (`dragToHighlight`, minmax-clamped, compared by
  displayedRegion index not refName).

### LinearComparativeView (abstract base of LinearSyntenyView et al.)

— `plugins/linear-comparative-view/src/LinearComparativeView/model.ts`

- `linkViews` (sync scroll/zoom in pixels), `followSynteny` (map other rows
  through the synteny data — mutually exclusive with linkViews),
  `sameScale` (one bpPerPx ceiling for all rows via LGV `sharedFitBpPerPx`),
  `followAnchorIndex`, `followMatchOrientation`, `levels: LinearSyntenyLevel[]`
  (one per adjacent pair — each level holds its own track list),
  `views: LinearGenomeView[]` (N rows, N−1 levels), `height`.
- Concrete subclasses (LinearSyntenyView, LinearReadVsRef,
  LinearDerivativeVsRef) add launch/init specifics and their synteny track.

### BreakpointSplitView — `plugins/breakpoint-split-view/src/BreakpointSplitView/model.ts`

- `height`, `showIntraviewLinks`, `linkViews`, `interactiveOverlay`,
  `showHeader`, `views: LinearGenomeView[]` (2 panels),
  `init` (transient child-panel spec). Persisted nav goes through the child
  LGVs.

### SpreadsheetView — `plugins/spreadsheet-view/src/SpreadsheetView/SpreadsheetViewModel.ts`

- `height`, `hideVerticalResizeHandle`, `importWizard` (ImportWizard model:
  adapter type/uri options), `spreadsheet: Spreadsheet?` (columns + row set;
  canals/column filters inside), `init`. Rows become regions via column
  typings ("loc" / refName+start+end columns); the view itself is a table, and
  the SvInspectorView projects it to regions.

### SvInspectorView — `plugins/sv-inspector/src/SvInspectorView/model.ts`

- `height`, `onlyDisplayRelevantRegionsInCircularView`,
  `spreadsheetWidthFraction` (0.66), `spreadsheetView: SpreadsheetView`,
  `circularView: CircularView` (embedded instances, `disableImportForm`),
  `init`. Sheet rows → circular chords; the two sub-views are ordinary MST
  children.

## 6. Session model

The shared base is `packages/product-core/src/Session/` (compose of mixins);
the web app composes it in `packages/web-core/src/BaseWebSession/index.ts`
`BaseWebSessionModel` and finalizes per product in
`products/jbrowse-web/src/sessionModel/index.ts` (`JBrowseWebSessionModel` =
BaseWebSessionModel + `WebSessionManagementMixin` + `permanentPlugins` volatile).

### Top-level field inventory (composed mixins → persisted props)

| mixin / file                                                                      | fields                                                                                                                                                                                                                                                                                                                           |
| --------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BaseSession.ts` `BaseSessionModel`                                               | `id`, `name`, `focusedViewId`, `highlightsVisible`, `heldForMissingPlugins: frozen HeldNode[]` (views/tracks stripped when a build lacks their plugin, with rehydration anchors); volatile `selection`, `hovered`, `queueOfDialogs`                                                                                              |
| `DrawerWidgets.ts` `DrawerWidgetSessionMixin`                                     | `drawerPosition` ('right', localStorage), `drawerWidth` (384, min-clamped int), `widgets: map(widget stateModel)`, `activeWidgets: map(safeReference(widget))`, `minimized`                                                                                                                                                      |
| `MultipleViews.ts` `MultipleViewsSessionMixin`                                    | `views: array(view stateModel)`, `stickyViewHeaders`, `useWorkspaces?: boolean`, plus workspace layout (per-view snapshots of the tabbed/tiled arrangement); focus tracked by `focusedViewId`                                                                                                                                    |
| `Tracks.ts` `TracksManagerSessionMixin`                                           | track show/hide/order actions, `addTrackConf` routing (admin→config, user→sessionTracks), config editor state                                                                                                                                                                                                                    |
| `SessionTracks.ts` `SessionTracksManagerSessionMixin`                             | `sessionTracks: array(track config)` (user-added tracks), `trackConfigDeltas: frozen Record<trackId, partial config>` (per-track **diffs** a non-admin made to catalog tracks; frozen so unset-vs-default survives), volatile per-track hydrated-config cache                                                                    |
| `Connections.ts` `ConnectionManagementSessionMixin`                               | `connectionInstances: array(connection stateModel)` (stripped from snapshots — holds the live fetched hub), `connectionTrackConfigs: frozen Record<trackId, entry>` (persisted configs of opened connection tracks so they resolve on load without re-connecting); getter `connections` reads admin config `jbrowse.connections` |
| `Preferences.ts`                                                                  | user preference overrides; promoted per-display-type defaults stored flat as `preferencesOverrides['displayTypeDefault\0<type>\0<slot>']`                                                                                                                                                                                        |
| `ReferenceManagement.ts`                                                          | reference/assembly session state                                                                                                                                                                                                                                                                                                 |
| `Themes.ts`, `TrackMenu.ts`, `TemporaryAssemblyTracks.ts`, `shareableSnapshot.ts` | theme manager, menu items, temporary assembly tracks (e.g. from search results), share-snapshot pruning                                                                                                                                                                                                                          |
| web-core `AppSessionMixin`                                                        | `sessionPlugins: array` (session-scoped plugin definitions), session assemblies, share helpers                                                                                                                                                                                                                                   |
| web `WebSessionManagementMixin` + `products/jbrowse-web/src/sessionModel`         | saved-session DB surface (favorites/recents/activate/delete), `permanentPlugins` (volatile mirror of localStorage — deliberately not shared)                                                                                                                                                                                     |

Widgets: each drawer widget is an MST node in `session.widgets` (a map), with
`activeWidgets` a `safeReference` map of which are open per drawer slot and
`visibleWidget` the one shown. Widgets hold `safeReference`s to views/tracks
(emptied on view close — `MultipleViews.ts` `dropRefsInto`).

### Session tracks vs catalog tracks

Catalog tracks live in admin config (`jbrowse.tracks` in config.json, owned by
`JBrowse-web`'s config model). `addTrackConf`: if the user is an admin it is
written to the catalog; otherwise the track is appended to `sessionTracks`.
User edits to a catalog track are `trackConfigDeltas[trackId]` — a partial
config diffed against the (still-current) admin base, so admin updates to
untouched fields still flow through. Track selection (which tracks a view
shows) is the per-view `tracks[]` array; the hierarchical track selector is
just a widget (`hierarchicalTrackSelector` id).

## 7. Session serialization & sharing

### Snapshot shape

A session snapshot is the MST snapshot of the session node, e.g. (from
`products/jbrowse-web/src/sessionShareRoundTrip.test.ts:38`):

```json
{
  "id": "…",
  "name": "My session",
  "views": [{ "id": "view1", "type": "LinearGenomeView", "bpPerPx": 10 }]
}
```

In practice a view snapshot carries: `type`, `windowStartBp`, `windowWidthBp`,
`displayedRegions[]`, `tracks[]` (each with `type`, `configuration.trackId`
reference, and display snapshots), toggles (`showGridlines`, `trackLabels`, …).
`trackSelectorType` and other vestigials are tolerated. `stripDefault` fields
are omitted when equal to default; LGV's `postProcessSnapshot` additionally
strips `init` (once displayedRegions exist), `showCenterLine`, and
localStorage-backed prefs unless non-default, and `preProcessSnapshot`
accepts legacy `offsetPx`/`bpPerPx` (converting to `windowStartBp =
offsetPx*bpPerPx`, `legacyBpPerPx`) plus old `showCytobandsSetting`/
`cytobandsVisible` names. There is no explicit version field; evolution is
`preProcessSnapshot` migrations plus `packages/product-core/src/sessionMigrations/`.

The **session spec** is the modern authoring surface (`agent-docs/reference/
SESSION_SPEC_FORMAT.md`): `views[]` entries carry launch keys flat
(`assembly`, `loc`, `tracks`, `displayedRegionNames`, `grow`, `highlight`)
under an `init` key rather than resolved state; a track entry is a bare
`trackId`, a tuple, or `{trackId, type, ...display slots inline}` — e.g.
`{"trackId":"hg002_ont","type":"LinearAlignmentsDisplay","colorBy":{"type":"tag","tag":"HP"},"height":400}`.
`normalizeTrackInit` (`packages/core/src/util/tracks.ts`) folds the inline
keys into the display snapshot. (`normalizeSnapshot` files found in plugins/
are adapter-config normalization, not session.)

### Share URLs (`packages/core/src/util/sessionSharing.ts`,

`products/jbrowse-web/src/components/buildShareUrl.ts`)

`encodeSessionParam(mode, session)` — three modes, three prefixes read by
`SessionLoader`:

- `short` → POST snapshot (URL-safe base64, AES-encrypted with a random
  5-char password that lives **only in the link**) to a share service; link
  carries `?session=share-<id>&password=…`.
- `long` → `?session=encoded-<urlsafe-b64(JSON)>` where the JSON is
  **deflate-compressed** (`toUrlSafeB64`, pako deflate) then url-safe base64;
  placed in the **hash fragment** so the server never sees it.
- `json` → `?session=json-<JSON>` plaintext (compact in link, pretty in dialog).

Admin params (`adminKey`, `adminServer`) and `password` are stripped from the
referer reported to the share service.

### Autosave

`products/jbrowse-web/src/rootModel/persistence.ts` + `sessionDbOps.ts`:
a debounced **400 ms** autorun writes the session snapshot into IndexedDB
(`sessions` + `metadata` stores, one transaction per write; newest-N pruning;
unload flushes). Session metadata rows carry favorite/recent flags; a delete
race with autosave is handled by read-modify-write in one transaction.

### grid-bookmark

`plugins/grid-bookmark/src/GridBookmarkWidget/model.ts`: a session widget
holding `bookmarks: LabeledRegionModel[]` (`{assemblyName, refName, start,
end, label, highlight color}` — regions 0-based half-open), a `gridView`
tab selector ('bookmarks'|'highlights'|'both'), persisted under a per-origin
localStorage key `bookmarks-<host><path>`. Bookmarks double as highlight
overlays in views (`highlightsVisible` session flag gates drawing).

## Mab notes

- **Store the viewport as a bp window, not pixels.** `windowStartBp` /
  `windowWidthBp` (with `bpPerPx` derived) is resize-proof and round-trip
  exact; JBrowse migrated to this precisely because pixel snapshots broke.
  Keep zoom clamp `[1/50 px/bp, totalBp/(width*0.9)]` and the 10/30 px edge
  paddings — they encode real UX constraints.
- **The layout is a plain array of `{refName, start, end, reversed}` regions,
  0-based half-open, always start<end.** A Rust `Vec<LayoutRegion>` with
  cumulative-bp indexing gives you pxToBp/bpToPx for free; keep the float
  conversions in f64 and subtract large offsets before f32 (GPU) — JBrowse's
  `staticBlocksTranslateX` comment documents the failure mode.
- **Separate static blocks (cache units, ~800 px chunks) from dynamic visible
  blocks (fetch windows)** and maintain a debounced "coarse" copy for
  expensive per-frame recomputes (autoscale). Fetch ±0.5 screen buffers.
- Navigation wants both levels: instant `setWindow` and an animated
  `flyTo` arc that is _cancellable by any other movement_ (compare what you
  wrote last frame). Prefer bp-space actions; the pixel ones exist only for
  gestures.
- Rubberband = two `BpOffset`s, selection materialized as whole-base regions
  on demand; nothing else is stored — a good minimal design.
- Search dispatch is a fixed precedence (refNames → glob → text search →
  locstring fallback); MAX_GLOB-style caps that _refuse rather than truncate_
  are worth copying.
- Session model is deliberately flat: one JSON tree, MST snapshots with
  omission-of-defaults, launch keys separated from state (`init` blob that is
  stripped after apply). A Rust equivalent: `serde` struct with `skip_serializing_if`
  defaults + a small migration pass on load; share via three encodings
  (server id + key, inline base64, inline JSON).
- Track customization deltas (`trackConfigDeltas`) rather than full copies
  keep admin updates flowing — same principle as patch-on-base for any
  catalog.
- No inter-region padding, no stored hideRegions/softClip: elision and seams
  are derived. Don't add viewport-adjacent state that a getter can compute.
- Ambiguities noted: (1) `HighlightType`/bookmark regions are 0-based in the
  model but entered/serialized via locStrings/URL that are 1-based — verify
  per entry point. (2) No formal session format version number; migrations
  are distributed (`preProcessSnapshot` + central `sessionMigrations`). (3)
  `encoded-` payloads are raw MST snapshots, so an evolved model field lands
  silently undefined on load unless a `preProcessSnapshot` migrates it.
