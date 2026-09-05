# 04 — Track model, display models, and the rendering pipeline

JBrowse 2 v4.3.0 (commit 83ac4507cf). Companion chapters cover adapters and views; this chapter covers what draws one track: the track state model, the display model family, the fetch→`rpcDataMap` pipeline, the three-backend renderer, and wiggle as the worked example. Map doc: `agent-docs/ARCHITECTURE.md` (authoritative for everything here; its `reference/` docs — GPU_RENDERING, FETCH_SKELETON, REGION_TOO_LARGE, DISPLAYCHROME, SVG_EXPORT — are the deep dives).

TL;DR: adapters fetch and parse in an RPC **worker**; the main thread stores results in an observable per-region map (`rpcDataMap`), **uploads** them once into GPU buffers, and **renders** every frame from those buffers. Pan/zoom is a redraw, not a refetch. Worker output is **absolute genomic uint32** (0-based half-open `[start, end)`, BED/BAM convention) — never pixels, never region-relative. Every canvas display also ships a Canvas2D draw function, which SVG export runs; the GPU path is an accelerator, not the floor.

## 1. Track model

### Two nodes per track: config and state

- **Track config** — a plain configuration node under `jbrowse.tracks[]` (catalog) or `session.sessionTracks[]`. Defined by the track type's `configSchema` (e.g. `plugins/linear-genome-view/src/FeatureTrack/configSchema.ts`); base slots come from `packages/core/src/pluggableElementTypes/models/baseTrackConfig.ts` (`baseTrackConfig`): `trackId`, `type`, `name`, `description`, `category`, `adapter` (a union of registered adapter schemas), `assemblyNames`, `displays[]` (one config per compatible display type, each with `type` + `displayId`), `textSearching.textSearchAdapter`, `formatDetails`/`formatAbout` (JEXL callbacks).
- **Track state** — created only when a track is _shown_, by `createBaseTrackModel(pm, trackType, configSchema)` (`packages/core/src/pluggableElementTypes/models/BaseTrackModel.ts`). Each plugin's track file (`plugins/*/src/*Track/index.ts`) registers one `TrackType` wrapping this factory, so every track type shares the state model. A track config can back multiple live instances.

`BaseTrackModel` field inventory (state tree node):

| field                 | kind     | notes                                                                       |
| --------------------- | -------- | --------------------------------------------------------------------------- |
| `id`                  | property | MST `ElementId` (uuid)                                                      |
| `type`                | property | `types.literal(trackType)`                                                  |
| `configuration`       | property | reference into the config tree (`ConfigurationReference`)                   |
| `minimized`, `pinned` | property | `types.stripDefault(boolean, …)` — omitted from snapshots when at default   |
| `displays`            | property | array of the plugin-union display state models; a shown track always has ≥1 |
| `resizing`            | volatile | a height-drag is in progress (displays drop expensive layers from frames)   |

Key getters: `trackId` (from config), `rpcSessionId` (= `adapterConfigCacheKey(adapterConfig)`, the **worker data-adapter cache key** and RPC routing id — the same adapter config is parsed once, shared by every track/display pointing at it), `adapterConfig`, `activeDisplay` (`displays[0]`), `adapterType`, `refNameMismatch` (diagnostic), `name`. The config schema declares multiple display configs; `replaceDisplay()` swaps the active one (radio in the track menu). On detach the track releases a refcount claim on its `rpcSessionId` so the worker can evict the adapter (`CoreFreeResources`).

### Catalog vs session tracks

`packages/core/src/util/types/index.ts`:

- **`addSessionTrackConf(conf)`** (`packages/product-core/src/Session/SessionTracks.ts`) — the default destination for a track a feature stands up on the user's behalf (search results, computed consensus). Persisted in `sessionTracks`, survives reload.
- **`publishTrackConf(conf)`** — Add-track workflows only: writes into the site catalog (`jbrowse.tracks`) with an admin's intent, for the whole site. Gate with the matching `isSessionWithAddSessionTrack` / `isSessionWithPublishTrackConf`; the deprecated `isSessionWithAddTracks` covers both.
- **`trackConfigDeltas`** (`trackId → partial config`, frozen map on the session) — when a user edits a **catalog** track's settings (quick `setConf` menu edits or the Settings dialog), the change is _diffed_ into this map rather than mutating the frozen catalog entry. That is what makes config slots per-instance _and_ persistent: the hydrated display config node is a detached scratch view over (base ∪ delta). ADR-032.

### Config → live state

`packages/core/src/util/tracks.ts` (the `addTrack` flow, called by `showTrack`): run `Core-preProcessTrackConfig` extension point → validate via `configSchema.create` → pick a display compatible with the containing view type (`pickDisplayForView`) → `trackType.stateModel.create({ type, configuration: trackId, displays: [{ type: displayType, configuration: displayId, ... }] })` → push onto `view.tracks`. Display-level settings in the snapshot are routed onto the display **config** node (slots), not the state node — a slot name written on a session display state node is dropped silently (the state model doesn't declare it).

### Where display state lives (the census that matters)

| home         | tag         | persists?               | note                                             |
| ------------ | ----------- | ----------------------- | ------------------------------------------------ |
| config slot  | `#slot`     | yes (in config / delta) | **the default**; nearly every track-menu setting |
| MST property | `#property` | yes (session snapshot)  | mostly just `type`, `configuration`              |
| MST volatile | `#volatile` | no                      | cached data, in-flight state, hover hits         |

20 registered displays declare 187 slots / 42 properties / 57 volatiles. `BaseDisplay`'s own volatiles: `error`, `statusMessage`, `statusProgress`.

## 2. Display model shape

### `BaseDisplay` (`packages/core/src/pluggableElementTypes/models/BaseDisplayModel.tsx`)

- properties: `id` (ElementId), `type` (display type name).
- volatiles: `error`, `statusMessage`, `statusProgress` (determinate fraction [0,1] or undefined).
- getters: `parentTrack`, `adapterConfig` (`getConf(parentTrack,'adapter')`), `isMinimized`, `RenderingComponent` (React component from pluginManager — the only mandatory React surface), `hoveredFeature` (overridable hover hook, `unknown` payload), `featureNoun` (default `'feature'`), `featureWidgetType` (default `BaseFeatureWidget`).
- methods: `renderingProps()`, `trackMenuItems()`, actions `setStatusMessage`, `setError`, `clearHoveredFeature()` (no-op hook), `reload()` (no-op hook).

### Composition: two LGV fetch foundations + comparative

All LGV displays compose `baseLinearDisplayConfigSchema` as their config base (`packages/core`); mixins supply the rest:

| foundation (`packages/display-kit/src/`) | composes                                                      | semantics                                                                                                                                                                                                                                                                                                                                                                                      |
| ---------------------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `MultiRegionDisplayMixin`                | `RegionTooLargeMixin` + `RenderLifecycleMixin` + `FetchMixin` | per-region fetch; **installs its four autoruns itself** in `afterAttach`; owns `loadedRegions` (a shallow `regionDataMap<LoadedRegion>` keyed by `displayedRegionIndex`), `fetchRegions()`, `isCacheValid(idx)`, `canvasWidthPx` (= `host.trackWidthPx` — the one legal canvas-width spelling). Used by wiggle, alignments, canvas, MAF, Manhattan, reference sequence, multi-row variant, GC. |
| `GlobalFetchMixin`                       | same three                                                    | one dataset for the whole viewport (Hi-C matrix, LD, arcs, multi-way synteny ribbons). Installs **no** autoruns; display supplies `prepare`/`run`/`commit` to `installGlobalFetchAutorun`. Owns `loadedFetchSignature` and `viewSignature` (block keys: `assembly:refName:start:end` joined — moves on block set change, not scroll).                                                          |

Comparative views (`LinearSyntenyDisplay`, `DotplotDisplay`) compose neither: `BaseDisplay` + `SyntenyFetchStateMixin` (`packages/synteny-core`) + `installComparativeFetchAutorun` (ADR-054). They also stack `RenderLifecycleMixin` _above_ several displays sharing one canvas, keyed by `sharedBackendKey(self.id)`.

Cross-cutting mixins (opt-in, `packages/display-kit/src/`): `TrackHeightMixin` (internal scroll, `scrollableHeight`), `HeightModeMixin` (fixed/grow/fit — must compose _after_ TrackHeightMixin), `ContextMenuMixin`, `LegendMixin`, `RowHeightMixin` (`rowHeight` slot; `0` = fit-to-display-height; resolved `effectiveRowHeight` getter), `ScoreScaleMixin` (scaleType/autoscale/minScore/maxScore/numStdDev), plus wiggle's `WiggleCommonMixin`.

`RenderLifecycleMixin` (`packages/render-core/src/RenderLifecycleMixin.ts`) owns: `canvasDrawn` (first-paint flag, test selectors), `currentRenderingBackend`, `renderTick` (render autorun trigger; bumped after every upload and by `renderNow()` on tab-visibility restore), `autorunsInstalled` guard, `renderError` (backend/context-loss error — the single source for the `renderError` phase), `canRender` (overridable; LGV overrides with `view.initialized`), and `attachRenderingBackend(backend, { upload, render })`.

## 3. Fetch pipeline

### Regions → RPC

`rpcManager.call(sessionId, 'WiggleGetMultiRegionStats' | 'CoreGetFeatures' | …, { adapterConfig, regions, bpPerPx, ...self.rpcProps(), stopToken, statusCallback, sessionId })` (core methods in `packages/core/src/rpc/methods/`: `CoreGetFeatures`, `CoreGetRegions`, `CoreGetSequence`, `CoreGetInfo`, `CoreGetRefNames`, `CoreGetRegionByteEstimate`, `CoreFreeResources`; display-specific ones live in plugins). Structural args (`adapterConfig`, `regions`, `bpPerPx`, `stopToken`) are spread at the call site; `rpcProps()` returns **user-controlled settings only** and is serialized (`serializeRpcProps`) into `rpcPropsCacheKey` — the cache key is the _serialized return value_, never the reads. Structural rules worth copying:

- **Never put a fetch-result derivative in `rpcProps()`** — infinite loop: fetch → derived value changes → key changes → `SettingsInvalidate` clears data → refetch. The trap and the fix (split user-controlled key from fetch-data views) are in ARCHITECTURE.md §"rpcProps loop trap".
- `sequenceAdapter` is derived by the RPC layer from the assembly already resolved (`renameRegionsIfNeeded`), never passed by the display.
- Regions crossing to the worker are **pre-canonicalized** (`renameRegionsIfNeeded`); a worker never canonicalizes refNames.
- Fetching is `sessionId`-keyed into the worker pool via `rpcSessionId` (adapter cache key), so a track's RPCs land on a sticky worker that has already parsed the adapter.

### The four fetch autoruns (per-region family)

`installPerRegionFetchAutoruns` (`packages/display-kit/src/installPerRegionFetchAutoruns.ts`), all `namedAutorun`s, first run on a **microtask** (leading edge — avoids issuing an RPC against un-configured state):

1. `DisplayedRegionsChange` — viewport content set changed → `clearAllRpcData()`.
2. `SettingsInvalidate` — serialized `rpcProps()` key changed → `clearAllRpcData()`.
3. `ClearBlockingStateOnViewportChange` — viewport moved while `error`/`fetchCanceled` set → clear them so the fetch autorun retries.
4. `FetchVisibleRegions` — viewport, `fetchGeneration`, or `reloadCounter` (immediate then debounced 600 ms) → for each visible block not covered by `isBlockCovered(...) && isCacheValid(idx)`, fetch it. While `regionTooLarge` holds, it re-fetches once per settled viewport so the gate re-measures (fetch stops at the measurement).

All four families run on the `runFetchOnce` latest-wins skeleton (`packages/core/src/util/installFetch.ts`): begin → clear error → run → commit **if still current** → `handleFetchError` → end. Cancellation: each fetch takes a stop token from a rotating pool; a new fetch cancels the old (`FetchMixin.activeStopToken`); **a user cancel is durable** — only Retry or (LGV) a viewport move re-arms. `fetchGeneration` bumps at every fetch end and is the re-evaluation epoch. Every family carries one unconditional "go again" signal read above every gate (per-region: `fetchGeneration`; global/comparative/other: `reloadCounter`) — a gate-consulted trigger read drops out of the autorun's dependency set and the autorun never wakes again.

### Cache/staleness (per-region)

`isCacheValid(idx) = regionHasData(idx) && (fetchKey stamp === regionFetchKey())`:

- `regionFetchKey` — what a fetch issued _now_ would produce. Wiggle: `String(view.bpPerPx)` (strict zoom equality — BigWig bins by zoom). Alignments: per-base bin over debounced `coarseBpPerPx`. Canvas: the amino-acid threshold. Default `''` (zoom-stable data).
- `regionHasData(idx)` — did the last fetch store anything (presence, not staleness; e.g. MAF keeps summary and detail tiers side by side under one index).
- `dataSuperseded` hook — the display's own inputs moved past the held data (export-readiness only; fails hung, never stale).

### `rpcDataMap`

Per-region results live in a display-owned volatile `rpcDataMap: regionDataMap<Result>('rpcDataMap')` — a **shallow** `observable.map<number, Result>` keyed by `displayedRegionIndex` (ADR-060; entries are never mutated, only set/cleared, so MobX deep observation is pointless). Storage is per-display-type: `WiggleCommonMixin` (`plugins/wiggle/src/shared/WiggleCommonMixin.ts`) declares `rpcDataMap: regionDataMap<WiggleDataResult>`; alignments declares `regionDataMap<GroupedAlignmentsResult>`; multi-wiggle derives its row list as a computed **first-seen union** over the map's values (`sourcesFromRegionData`) instead of keeping a second store. The upload side optionally reads a _derived_ map (e.g. alignments' `laidOutByGroup`: per-group shallow clones with main-thread-computed Y arrays) so settings that reshape data re-upload without refetching.

## 4. Rendering pipeline

```
worker: adapter → typed arrays (absolute uint32 bp) ──RPC──▶ main
main: rpcDataMap (MST volatile, shallow observable map)
        │ upload autorun (fires when data changes) — pushes bytes to GPU buffers
        ▼
      GPU buffers ── render autorun (fires on any visible change) ──▶ <canvas>
SVG export re-runs the Canvas2D draw fn on an offscreen canvas → <image>/paths.
```

### The autorun pair

A display calls `self.attachRenderingBackend(backend, () => ({ upload, render }))` inside its `startRenderingBackend(backend)` action (the thunk runs once, on first attach; re-fired on GPU context-loss recovery). MobX auto-tracks every observable read inside each callback — no declared dependencies. `upload(backend) → boolean` ("did anything reach the backend"; `true` forces a redraw). `render(backend) → boolean` ("did I draw"; flips `canvasDrawn`). React is a thin bridge: `useRenderingBackend(model, canvasRef)` creates the canvas/ctx, hands the backend to the model, and `DisplayChrome` hosts it. Upload is **keyed and diffed** (`installUpload`, `mapUploadSync`): only changed regions re-pack/re-upload.

### Backend HAL selection

`packages/render-core/src/createRenderingBackend.ts` builds a runtime ladder: **WebGPU → WebGL2 → Canvas2D** (`createGpuHal` in `packages/render-core/src/hal/`: `webgpuHal.ts`, `webgl2Hal.ts`, shared `gpuHalBase.ts`, an OOM reporter, device caches, `mockHal` for tests). Each rung's failure reason is collected into an `AggregateError` if even Canvas2D fails. Both GPU backends implement one `GpuHal` interface (`hal/types.ts`: pipelines from `PipelineDescriptor[]`, uniform blocks, MSAA sample count default 4 — per-display, costed per track). A Canvas2D-only display skips the ladder via `createCanvas2DBackend` (e.g. `plugins/sequence`'s `SequenceRenderer`). Every display provides a `Canvas2DXxxRenderer` regardless; GPU is an accelerator.

### Shaders

`packages/shader-tools/src/build-shaders.ts` compiles every `*.slang` in the workspace (`pnpm gen:shaders`) into `*.generated.ts` artifacts: WGSL **and** GLSL-ES-300 strings, plus reflection-derived layouts (stride, field offsets, typed packers, `VERTEX_ATTRIBUTES`) — the generated file is the single source of truth for buffer layouts; TS imports constants by name. Directives: `//! targets:`, `//! export-consts:` (shared constants, e.g. `MIN_FILL_WIDTH_PX`), `//! js-export:` (a TS twin of a shader function, kept as a parity oracle, e.g. wiggle's `makeScoreNormalizer`), `//! layout-out/consts-out/js-export-out:`. Never hand-edit `*.generated.ts`. Shared geometry lives in `packages/render-core/src/shaders/` (`rowRect.slang` composed rect shape, `pointGlyph`, `hpmath` hi/lo float pair math, `colorPack`, `scoreScale`).

### SVG export

`packages/render-core` + each display's `renderSvg.tsx`: `renderDisplaySvg(model, opts, Body)` awaits `svgReady` (fails the whole export if one track can't settle; only bound is a 30-minute backstop, so every resting state must be terminal: `error` / `regionTooLarge` / `fetchCanceled` / `fetchInert`), then mounts `SvgChrome` and paints via the display's Canvas2D draw functions at the shell's `canvasWidth`. On-screen and exported pixels can't drift because they are the same code; the shader is never in the export path. Arc displays skip canvas entirely — JSX `<path>`s on both paths.

### Chrome and phases

`computeDisplayPhase({ renderError, regionTooLarge, error }, loading)` (`packages/render-core/src/displayPhase.ts`) — precedence `renderError > tooLarge > error > loading > ready`; `DisplayStatusPhase` = `tooLarge|error|loading|ready` (backend-free), `DisplayPhase` adds `renderError`. `loading` is a **thunk**, evaluated only after terminals are ruled out (keeps the observer's dependency set small while a banner is up). `DisplayChrome` (`packages/display-kit/src/DisplayChromeBase.tsx`) renders the phase: `renderError`/`tooLarge` **early-return as the entire root** (unmounts the canvas → clean backend dispose; force-load remounts and re-inits); `error`/`loading` are overlays over the still-mounted canvas. It also owns the pointer-position measurement that hit-testing reads.

### The "region too large" gate

`RegionTooLargeMixin` (`packages/display-kit/src/RegionTooLargeMixin.ts`): the display opts in with `gateEnabled` and passes `byteLimit: self.resolvedByteLimit()` (from the `fetchSizeLimit` slot) in its feature RPC. **The feature RPC itself measures bytes**; the worker stops at the limit and reports an estimate instead of downloading. `regionTooLarge` is a _derived_ getter over that last measurement, plus a density axis (`densityTooLarge`, canvas `CanvasFeatureGateMixin`). The banner releases itself by continuing to fetch once per settled viewport with the fetch stopping at the measurement — no imperative clear, no flicker on pan. `forceLoad` config slot / `forceLoadTrack` action bypass.

## 5. Wiggle case study (`plugins/wiggle`)

Two displays (`LinearWiggleDisplay`, `MultiLinearWiggleDisplay`) over three shaders, one Canvas2D twin, one hit test — shared machinery in `src/shared`, scale/axis in `packages/wiggle-core` (six other plugins draw a wiggle-shaped axis against it).

- **Composition**: `LinearWiggleDisplay/model.ts` = `TrackHeightMixin() + MultiRegionDisplayMixin() + WiggleCommonMixin()` + config schema. `WiggleCommonMixin` (which extends `WiggleScoreConfigMixin` ⊃ `ScoreScaleMixin`) owns `rpcDataMap: regionDataMap<WiggleDataResult>`, the stored hover (`hoveredWiggleFeature` volatile + `hoveredFeature` getter), strict-`bpPerPx` `regionFetchKey`, autoscale domain, and clear action.
- **RPC result** (`packages/wiggle-core/src/dataTypes.ts` `WiggleDataResult`): `sources: WiggleSourceData[]`, each `{ name, color?, labelColor?, label?, group?, baseUri? }` + typed arrays `featurePositions: Uint32Array` (interleaved start/end, absolute 0-based half-open bp), `featureScores: Float32Array`, `featureMinScores/featureMaxScores`, `pos*`/`neg*` splits for bicolor, `numFeatures`, `hasSummaryScores`. Shipped arrays are deliberately **aliased** (read-only; structured clone preserves sharing; `collectWiggleTransferables` dedupes transferables across regions).
- **Rendering types**: xyplot, scatter, density, line, linecenter, whiskers (filled splits into solid layers only when bands nest, back-to-front from the pivot). Record layouts: fill record 20 bytes (`WiggleFillInstance`, `wiggleCommon.slang`) feeding `wiggle.slang` + `wiggleDensity.slang` (density = render-core `rowRect` shape via `bufferPassId`); line record 40 bytes for `wiggleLine.slang`. Each pass packs its own buffer and returns **empty** for renderings not its own. The pass/buffer/`renderingType` uniform/Canvas2D painter all come from the **encoded layers**, never from `renderState` — a plot-type switch draws the previous plot for one frame by design.
- **Coloring model**: `useBicolor` (default true) → `posColor`/`negColor` around `bicolorPivot`; the worker owns the avg-path pos/neg split, whisker bands are colored main-thread. Density uses `densityColorRamp` (256-entry LUT; a change is one uniform flag + LUT texture upload, never a refetch). `effectiveSummaryScoreMode` (whiskers→`avg` under density) drives autoscale/menu/tooltip/gpuProps — but **`rpcProps()` carries the raw slot**, so switching rendering type doesn't re-download. Color change → re-encode only; `bicolorPivot` change → worker output differs → refetch.
- **Geometry**: `plotGeometry { yTop, plotHeight, numRows, tickHeight }` states single-row-inset vs multi-row-stacked once; everything (ticks, render state, canvas box, SVG clip) reads it. `rowIndex` is the position in the display's own `sources`, never the payload's; a missing source leaves its row empty. Gap rules: step line breaks on bp adjacency; `linecenter` breaks only past `gapLimitBp` (bp, not px); default gap break multiple is off. Bar floor `MIN_FILL_WIDTH_PX` shared both backends; the 0.8 px Canvas2D fudge is Canvas2D-only (AA compensation, no shader twin).

## 6. Feature hover / selection

- **Hover**: `DisplayChrome` measures the pointer against one container rect and calls the display's handler; a display resolves the hit in main-thread state and publishes it via the `hoveredFeature` hook (`BaseDisplay`). Wiggle derives it fresh each pointer event via `wiggleMouseHandlers` (`plugins/wiggle/src/shared/wiggleMouseHandlers.ts`) + `computeHit` → binary search (`findFeatureAtBp`) over the sorted `featurePositions` typed array. Storing displays (canvas, alignments, variants) keep the hit in a differently-named volatile with a getter over it (MST refuses a volatile over a base computed) and must clear it on viewport change (`installClearHoverOnViewportChange`) — a stored hit outlives the pixels it named under zoom/pan/scrollTop/banner. `LinearGenomeViewContainer` publishes `hoveredFeature` to `session.hovered`.
- **Click**: resolved **from the click event itself**, not the last hover (the viewport can move under a stationary cursor); `model.selectFeature(feat)` runs the display's selection action — the feature widget opens via `openFeatureWidget` (`packages/core/src/util/openFeatureWidget.ts`) using `featureWidgetType` (`{ type, id }`; shared `id` = shared drawer panel), and the session's selection set records it. The generic widget fetches the full feature by id over RPC when the on-screen payload is a reduced summary.

## Mab notes

Transferable concepts:

- **Fetch-then-render split.** Parse off the render thread; store immutable, absolutely-coordinate results keyed by viewport region; treat rendering as a pure function of (data, viewport, settings). The "absolute uint32 genomic coordinates in, pixels only at draw time" rule removes an entire class of region-relative bugs and makes data cacheable across zooms.
- **Separate cache-invalidation axes.** JBrowse's cleanest idea: `rpcProps` (user settings → global invalidation, keyed on serialized payload) vs `regionFetchKey` (per-region content axis, e.g. zoom bin) vs `regionHasData` (presence) vs `dataSuperseded`. Copy the split; Rust trait equivalents: `fn rpc_props(&self) -> SettingsKey`, `fn region_fetch_key(&self) -> ContentKey`, `fn region_has_data(&self, idx) -> bool`.
- **Display phases as one computed enum** (`renderError > tooLarge > error > loading > ready`) with one renderer-side switch — beats per-display boolean soup. Also: every "never fetches" resting state must be terminal, or readiness gates hang.
- **Byte/density gate measured in the data fetch itself** (worker stops at the limit, derived verdict, self-releasing banner) — cheap to port, big UX win.
- **Draw-fn floor + accelerator.** Ship a 2D-CPU draw path (also your SVG/export path) and treat a GPU path as an optional accelerator over shared constants; derive both from one geometry/pari table to prevent drift.
- **Upload once, redraw per frame.** Buffer upload diffed per region; pan/zoom = redraw with new uniforms. Wiggle's autoscale-pan cost being "one uniform write, zero buffer bytes" is the target.
- **Per-region payload typed arrays with interleaved position/score layout** (wiggle's `Uint32Array`/`Float32Array` convention) map directly onto Rust `Vec<u32>`/`Vec<f32>` + GPU vertex attributes; the 20-vs-40-byte split record ("only stroked renderings read a neighbour") is a good buffer-layout discipline.
- **Row order is a main-thread concern**: fetch names the row _set_, placement applies order locally — reorder/cluster becomes a re-upload, not a refetch.
- **Cancel semantics**: latest-wins with durable user cancel and a retry counter; stop tokens per fetch.

Web-specific plumbing (do **not** port):

- MobX autoruns, MST snapshot/volatile machinery, `trackConfigDeltas` diff persistence, JEXL config callbacks, React chrome components, the WebGPU/WebGL2 HAL, Slang codegen (Mab can generate WGSL/spirv from its own pipeline), the RPC worker pool (a native thread pool + serde frames is the analog), `getConf` slot resolution with promotable defaults.

Ambiguities worth flagging: `displayedRegionIndex` semantics depend on the LGV block model (a Rust port should define its own stable region-id scheme up front); the exact per-display `Canvas2DXxxRenderer`/`GpuXxxRenderer` API differs per plugin (there is no single renderer trait, only the HAL contract + per-display callbacks); session-vs-catalog persistence (`trackConfigDeltas`) is heavily product-layer and likely not worth mirroring 1:1.
