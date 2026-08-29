# 05 — Adapters, RPC, and Data Loading

TL;DR: JBrowse's data layer is a uniform **adapter contract** (getFeatures as an
RxJS Observable over a region) executed inside **RPC workers**, with
one-instance-per-config adapter caching per worker, per-region byte-estimate
gating measured from the file's _index_ before any feature download, and
cancellation via stop-tokens. All feature output crosses the worker boundary as
0-based half-open `[start, end)` absolute genomic coordinates.

Source: JBrowse 2 v4.3.0, commit 83ac4507cf.

---

## 1. The adapter contract

`packages/core/src/data_adapters/BaseAdapter/BaseAdapter.ts` — `BaseAdapter` is
a plain (non-MST) class. Fields:

| field                   | role                                                                                                                    |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `config`                | the adapter's MST configuration model (typed `CONF`)                                                                    |
| `getSubAdapter`         | factory `(adapterConfigSnap) => cached adapter` for adapters that need sub-adapters (e.g. BAM needs a sequence adapter) |
| `pluginManager`         | for opening filehandles via `openLocation`                                                                              |
| `id`                    | from `getAdapterId(config)` — the config's `adapterId`, hash of config otherwise                                        |
| `sequenceAdapterConfig` | stashed once (`??=`) for reference-decoding adapters (CRAM binds `seqFetch` at construction)                            |
| `getConf(slot)`         | readConfObject shorthand                                                                                                |

Derived base classes (`BaseAdapter/index.ts`):

- **`BaseFeatureDataAdapter`** (`BaseFeatureDataAdapter.ts`) — the common case.
  Abstract methods:
  - `getRefNames(opts?): Promise<string[]>` — names used in the file (need not
    match assembly names; main thread maps aliases).
  - `getFeatures(region: Region, opts?: BaseOptions): Observable<Feature>` —
    **0-based half-open** region, lazy pull-based stream.
    Provided helpers (all delegate to the two above):
  - `getHeader(): Promise<unknown>` — file header (BAM @SQ tags, VCF header
    text, etc.); `{tag,data}[]` or nested JSON.
  - `getMetadata()` — info for feature-detail panels (parsed VCF header
    metadata).
  - `getFeaturesInMultipleRegions(regions, opts)` — merges per-region
    observables (RxJS `merge`), each with its own status slot.
  - `getFeaturesArray` / `getFeaturesInMultipleRegionsArray` — collect to
    arrays; the shape every RPC method uses.
  - `hasDataForRefName(refName)`.
  - `getRegionQuantitativeStats(region)` / `getMultiRegionQuantitativeStats` —
    min/max/mean etc. via `scoresToStats` (`util/stats.ts`), run with
    `statsEstimationMode: true` (adapters may sample).
  - `getRegionByteSize(regions, opts): Promise<number | undefined>` — **index-only
    compressed-byte estimate**, no feature parse. Default `undefined` (= no byte
    gate for that adapter). Overridden by tabix-family and BigBed adapters; not
    by BigWig/sequence/HiC because they cap output at screen resolution.
  - `getSources(regions)` — derive named series (multi-wiggle, multi-sample).
  - `getExportData(regions, formatType)` — direct raw-text export (VCF tabix
    and BAM can export without re-parsing).
  - `getFeaturesInMultipleRegions` concurrency + per-region status fan-out via
    `createStatusFanOut`.
- **`BaseSequenceAdapter`** — extends feature adapter, implements
  `RegionsAdapter` (`getRegions(opts): Promise<NoAssemblyRegion[]>`), plus
  `getSequence(region)`. Deliberately **no `getRegionByteSize`** (output is
  capped at screen resolution).
- **`BaseRefNameAliasAdapter`** — supplies refName alias tables (see chapter
  01 and chapter 03).
- **`BaseTextSearchAdapter`** — `searchIndex(args: BaseTextSearchArgs):
  Promise<BaseResult[]>`; args = `{queryString, searchType: 'full'|'prefix'|'exact',
  stopToken}`. No pagination: adapters return all matches, the manager ranks.

**Region parameter shape** (`packages/core/src/util/types/data.ts`):
`Region = { assemblyName, refName, start, end, reversed }`;
`AugmentedRegion` adds `originalRefName` (worker-side renaming already applied —
adapters never canonicalize refNames themselves, see
`agent-docs/reference/REFNAME_NAMESPACES.md`). `NoAssemblyRegion` omits
assemblyName. Coordinates are **0-based, half-open, interbase** throughout.

**Options** (`BaseAdapter/types.ts` `BaseOptions`) — the full fetch-context bag:

| field                                 | notes                                                                       |
| ------------------------------------- | --------------------------------------------------------------------------- |
| `stopToken`                           | cancellation handle (see §3)                                                |
| `bpPerPx`, `resolution`               | zoom; adapters choose zoom levels themselves (BigWig, HiC, PIF coarse/fine) |
| `sessionId`, `trackInstanceId`        | cache scoping / status                                                      |
| `signal`                              | AbortSignal passthrough for @gmod libs                                      |
| `statusCallback`                      | out-of-band progress (string label, or `{current,total}` determinate)       |
| `headers`                             | per-request HTTP headers                                                    |
| `statsEstimationMode`                 | adapter may sample for stats                                                |
| `assemblyName` / `targetAssemblyName` | which side of a comparative pairing                                         |
| `lodMode: 'fine'                      | 'coarse'`                                                                   |

**Adapter registration & config schema**: each adapter ships a
`configSchema.ts` built with `ConfigurationSchema(...)` (typed slot tree with
defaults, descriptions) and registers via an `AdapterType`
(`packages/core/src/pluggableElementTypes/AdapterType.ts`): `{name,
getAdapterClass (async import), configSchema, adapterCapabilities,
description?, alsoReads? (an Add-track hint only)}`. Capabilities are free-form
strings (`'getFeatures'`, `'getRefNames'`, `'hasResolution'`, `'exportData'`).
Adapter _guessing_ (`Core-guessAdapterForLocation`) is a first-match-wins chain
over file extension/name patterns. Config examples:

- `BamAdapter` config (`plugins/alignments/src/BamAdapter/configSchema.ts`):
  `bamLocation`, `index: {indexType: 'BAI'|'CSI', location}`, `fetchSizeLimit`
  (default **5,000,000 bytes**, `advanced`), plus sequence-adapter slot.
- `VcfTabixAdapter`: `vcfGzLocation`, `index: {indexType, location}`.

## 2. Adapter inventory

All listed adapters are `BaseFeatureDataAdapter` subclasses unless noted.
"Byte-est" = overrides `getRegionByteSize` (participates in the too-large gate).

| Adapter (type string)                                                                                                                                                                  | Package                      | Format                | Index                          | Queryable        | Byte-est                                | Notes                                                                                                                                                                                                          |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------- | --------------------- | ------------------------------ | ---------------- | --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `BamAdapter`                                                                                                                                                                           | plugins/alignments           | BAM                   | BAI/CSI (.bai/.csi)            | yes              | yes                                     | `@gmod/bam`; lazy features (`BamSlightlyLazyFeature`); BGZF inflate spread over `sharedBgzfWorkerPool()` (4 workers, measured 1.95x)                                                                           |
| `CramAdapter`                                                                                                                                                                          | plugins/alignments           | CRAM                  | CRAI                           | yes              | yes                                     | `@gmod/cram`; needs sequence sub-adapter for reference restore; separate codec pool; shared per-context record-count budget                                                                                    |
| `HtsgetBamAdapter`                                                                                                                                                                     | plugins/alignments           | htsget BAM            | server                         | yes              | —                                       | extends `BamAdapter`, swaps the reader for `@gmod/bam`'s `HtsgetFile`; config: `htsgetBase` + `htsgetTrackId`                                                                                                  |
| `SamAdapter`                                                                                                                                                                           | plugins/alignments           | SAM (text)            | none                           | whole-file scan  | no                                      | line parse, filters by refName/start                                                                                                                                                                           |
| `VcfTabixAdapter`                                                                                                                                                                      | plugins/variants             | VCF                   | tabix TBI/CSI                  | yes              | yes                                     | `@gmod/tabix` + `@gmod/vcf`; `getExportData('vcf')` streams raw lines; shared BGZF pool                                                                                                                        |
| `VcfAdapter`                                                                                                                                                                           | plugins/variants             | VCF (plain)           | none                           | whole-file       | no                                      |                                                                                                                                                                                                                |
| `SplitVcfTabixAdapter`, `PlinkLDAdapter`                                                                                                                                               | plugins/variants             | VCF / PLINK .ld       | tabix                          | yes              | —                                       | PlinkLD computes LD matrices over binned windows                                                                                                                                                               |
| `Gff3TabixAdapter` / `Gff3Adapter`                                                                                                                                                     | plugins/gff3                 | GFF3                  | tabix / none                   | yes              | yes (tabix)                             | `@gmod/gff` parser; NCList-style lazy feature trees from GFF3Tabix                                                                                                                                             |
| `GtfTabixAdapter` / `GtfAdapter`                                                                                                                                                       | plugins/gtf                  | GTF                   | tabix / none                   | yes              | yes (tabix)                             |                                                                                                                                                                                                                |
| `BedTabixAdapter`, `BedGraphTabixAdapter`, `BedAdapter`, `BedGraphAdapter`, `BedpeAdapter`, `StarFusionAdapter`                                                                        | plugins/bed                  | BED family            | tabix (indexed variants)       | yes              | yes (Bed/BedGraph tabix)                | `@gmod/bed` parser                                                                                                                                                                                             |
| `BigBedAdapter`                                                                                                                                                                        | plugins/bed                  | BigBed                | internal B+ tree               | yes              | yes                                     | `@gmod/bbi-js`; `getRegionByteSizeMulti`                                                                                                                                                                       |
| `BigWigAdapter`                                                                                                                                                                        | plugins/wiggle               | BigWig                | internal B+ tree + zoom levels | yes              | no (output capped at screen resolution) | `@gmod/bbi-js`; returns **typed arrays** (`getFeatureArrays`/`getFeatureArraysMulti` with `regionOffsets` slicing, coalesced across regions); picks a bbi **zoom level** from `bpPerPx × resolutionMultiplier` |
| `TwoBitAdapter`                                                                                                                                                                        | plugins/sequence             | .2bit                 | internal                       | yes              | no                                      | `@gmod/twobit`; optional `chromSizesLocation` fast path                                                                                                                                                        |
| `IndexedFastaAdapter` / `BgzipFastaAdapter`                                                                                                                                            | plugins/sequence             | FASTA                 | .fai (+ .gzi for bgzip)        | yes              | no                                      | shared `FastaAdapterBase`; `@gmod/indexedfasta`                                                                                                                                                                |
| `UnindexedFastaAdapter`                                                                                                                                                                | plugins/sequence             | FASTA                 | none                           | whole-file       | no                                      |                                                                                                                                                                                                                |
| `ChromSizesAdapter`                                                                                                                                                                    | plugins/sequence             | chrom.sizes text      | none                           | regions only     | —                                       | implements **`RegionsAdapter`** (getRegions only); used to define assemblies cheaply                                                                                                                           |
| `FromConfigAdapter` / `FromConfigRegionsAdapter` / `FromConfigSequenceAdapter`, `RefNameAliasAdapter`, `NcbiSequenceReportAliasAdapter`                                                | plugins/config               | inline JSON           | —                              | trivial          | —                                       | features/regions/aliases from config itself                                                                                                                                                                    |
| `HicAdapter`                                                                                                                                                                           | plugins/hic                  | Hi-C (.hic)           | internal                       | yes              | no                                      | `straw`-style reader; zoom-normalized contact matrices                                                                                                                                                         |
| `MafTabixAdapter`, `BgzipMafAdapter`, `BgzipTaffyAdapter` (TAF/taffy + custom .tai index), `BigMafAdapter`                                                                             | plugins/maf                  | MAF                   | tabix / bgzip blocks / TAI     | yes              | —                                       | `BgzipTaffyAdapter` streams TAF blocks with run-length-encoded bases; MAF display has a **summary tier** (see §5)                                                                                              |
| `GWASAdapter`                                                                                                                                                                          | plugins/gwas                 | BED-like GWAS         | tabix                          | yes              | —                                       | **extends `BedTabixAdapter`**                                                                                                                                                                                  |
| `CrisprGuideAdapter`, `MotifListAdapter`                                                                                                                                               | plugins/sequence             | —                     | scan of reference              | yes              | —                                       | derive from `ReferenceScanAdapter` (computed, not stored)                                                                                                                                                      |
| `GCContentAdapter`                                                                                                                                                                     | plugins/gccontent            | computed              | —                              | yes              | —                                       | computes GC from sequence sub-adapter                                                                                                                                                                          |
| `PAFAdapter`, `PairwiseIndexedPAFAdapter` (PIF), `AllVsAllPAFAdapter`, `AllVsAllIndexedPAFAdapter`, `ChainAdapter`, `DeltaAdapter`, `MashMapAdapter`, `MCScan*`, `BlastTabularAdapter` | plugins/comparative-adapters | synteny formats       | PIF index (.pif.gz + tbi)      | yes              | —                                       | PIF has fine/coarse **LOD tiers** selected by `lodMode`                                                                                                                                                        |
| `NCListAdapter`, `JBrowse1TextSearchAdapter`                                                                                                                                           | plugins/legacy-jbrowse       | JBrowse 1 NCList      | binary NCList blobs            | yes              | —                                       | legacy read path                                                                                                                                                                                               |
| `SPARQLAdapter`                                                                                                                                                                        | plugins/rdf                  | RDF endpoint          | server                         | yes              | —                                       |                                                                                                                                                                                                                |
| `TrixTextSearchAdapter`                                                                                                                                                                | plugins/trix                 | trix index (.ix/.ixx) | TriX                           | text search only | —                                       | `searchIndex`; in-memory suffix-ordered index                                                                                                                                                                  |
| `JBrowse1TextSearchAdapter`                                                                                                                                                            | plugins/legacy-jbrowse       | JBrowse1 names index  | —                              | text search only | —                                       |                                                                                                                                                                                                                |

**No "jbrowseread" adapter exists in-tree** (the string appears only in
products/jbrowse-capture and test utils for reading local JBrowse sessions —
not a data adapter). The spreadsheet-view plugin is a UI for CSV/Excel that
runs through `FromConfigAdapter`-style in-memory data, not a genomic adapter.
Ambiguity: if "jbrowseread" meant the JBrowse 1 read path, that is
`NCListAdapter`.

**Coordinate convention**: uniformly 0-based half-open `[start,end)`, matching
BED/BAM internally; text formats that are 1-based (GFF3, VCF) are converted at
parse time.

## 3. RPC mechanics

Path: display (main thread, MST model) → `RpcManager.call(sessionId,
functionName, args)` → driver → worker → `RpcServer` → method `execute`.

**Drivers** (`packages/core/src/rpc/`):

- `RpcManager` (`RpcManager.ts`) — one per session; resolves driver from config
  `defaultDriver` or host default (`WebWorkerRpcDriver` for web/desktop,
  `MainThreadRpcDriver` for embedded/headless). **No per-call/per-track driver
  override exists** (deliberately removed; ADR-086).
- `BaseRpcDriver` (`BaseRpcDriver.ts`) — envelope: look up the
  `RpcMethodType` by name, strip `statusCallback` from args (it is an
  out-of-band handle, never serialized), `rpcMethod.serializeArguments(args)`
  (this is where refName maps get resolved — which can download the index — so
  the status callback is stripped _after_ serialization), check stopToken
  before and after (via `withStopTokenCheck`), then `transport(...)`, then
  `deserializeReturn`. `freeSession(sessionId)` dispatches `CoreFreeResources`.
- `WebWorkerRpcDriver` (`WebWorkerRpcDriver.ts`) — a **pool of LazyWorkers**,
  size = config `workerCount` or `clamp(hardwareConcurrency−1, 1, 5)` (so ≤5
  workers, typical 4). Lazy workers boot on first use; a dead worker invalidates
  its slot and re-boots. Worker is booted with the runtime plugin list,
  `windowHref`, and `numberGrouping` (string formatting preference). The
  `Core-extendWorker` extension point lets a plugin wrap the worker handle
  (request/response of its own across the boundary, e.g. Apollo plugin asking
  the main thread for sequence).
- `MainThreadRpcDriver` — runs methods in-band; used in Node/headless and
  embedded defaults.
- `RpcClient`/`RpcServer` — the wire protocol: frames `{uid, data|error|
  eventName|libRpc}`; errors serialized via `serializeError/` (structured error
  objects, not strings). A `messageerror` (undeserializable frame) rejects **all**
  pending calls on that worker.
- **Status side-channel**: progress is posted on a per-call named channel
  (`message-<id>` events), never inside the reply. No `statusCallback` on the
  call means no channel is minted at all.
- **Transferables**: worker results that own ArrayBuffers are returned as
  `rpcResult(value, transferables)` / `rpcResultWithArrayBuffers(value)`
  (`RpcServer.ts`) — zero-copy move of typed arrays via postMessage transfer
  list, with dev/test verification that the transfer list matches the payload.
  This is how wiggle/canvas/synteny packers ship megabytes cheaply.

**Core RPC methods** (`packages/core/src/rpc/methods/`, registered via
`coreRpcMethods.ts`; plugins register more via `RpcMethodType`):

| method                              | does                                                                                          |
| ----------------------------------- | --------------------------------------------------------------------------------------------- |
| `CoreGetFeatures`                   | adapter `getFeaturesInMultipleRegions` → `Feature.toJSON()` array (`SimpleFeatureSerialized`) |
| `CoreGetRefNames`, `CoreGetRegions` | adapter refNames / region list                                                                |
| `CoreGetInfo`, `CoreGetMetadata`    | getHeader / getMetadata                                                                       |
| `CoreGetSequence`                   | sequence sub-adapter fetch                                                                    |
| `CoreGetRegionByteEstimate`         | index-only byte estimate (used by save/export dialogs, not the display gate)                  |
| `CoreGetExportData`                 | raw text export                                                                               |
| `CoreFreeResources`                 | drop this session's claims on cached adapters (`freeAdapterResources`)                        |

`RpcMethodTypeWithRenameRegions` is the common base for the feature methods: it
maps incoming region refNames through the assembly's refName aliases to the
file's namespace before `execute` (the main thread hands canonical assembly
names; the adapter sees file-native names).

**Cancellation — stop tokens** (`packages/core/src/util/stopToken.ts`): a token
id string; stopping records the id locally and broadcasts to every booted
worker, whose `RpcServer` adds it to a set — checks are set lookups at
`await` points. For loops with no awaits, a synchronous check exists:
SharedArrayBuffer atomic flag (only when COOP/COEP cross-origin isolation —
**none of the shipped deployments use it**), otherwise a revocable-blob-URL
synchronous XHR probe, throttled. `checkStopTokenThrottled` is wired into the
byte-measurement path. Aborts surface as ordinary `AbortError`s that callers
treat as the normal outcome of a superseded fetch.

**PhasedScheduler** (`packages/core/src/PhasedScheduler.ts`): not RPC-related
despite the name — it runs plugin-registration callbacks in declared phase
order and aggregates constructor errors. Nothing to do with data loading.

**jobs-management plugin** (`plugins/jobs-management`): a UI widget (JobsList)
over a session jobs list; no worker queuing machinery. Cancel/progress of data
fetches lives in the stop-token + statusCallback channels instead.

## 4. Caching, gating, dedup

**Adapter cache** (`packages/core/src/data_adapters/dataAdapterCache.ts`) —
lives in whichever JS context instantiates adapters (i.e. per RPC worker, plus
main thread for MainThread driver). Key = `adapterId` (or hash of config
snapshot). One adapter instance per config for the life of the track; the
cache holds a `Promise<AdapterCacheEntry>` (dedups concurrent instantiation),
entry tracks a `Set<sessionId>`; `CoreFreeResources` decrements and deletes
unclaimed entries. Rejected promises self-evict. **No size bound by design** —
reclamation relies on GC; note that @gmod/bam/tabix/cram keep parsed chunks in
`SharedReadCache`s that self-sweep on a timer (~3 min), so memory lags the
close.

**Chunk/byte budgets** (`packages/core/src/util/cacheBudgets.ts`): two
_shared_ budgets per JS context rather than per file —
`decompressedBytesBudget = SharedBudget(1 GiB)` (BAM, tabix) and
`decodedRecordsBudget = SharedBudget(1,000,000 records)` (CRAM only, because
records have no cheap byte size). Rationale (measured): per-file ceilings
multiplied by track count until nothing bounded the sum; per-file divisions
fell off the "smaller than one query's working set" cliff.

**Region/data caches on the display side** (not adapter side): the display
models store per-region RPC results in an MST `rpcDataMap` keyed by region
index and invalidated by viewport changes, `rpcProps()` (user settings that
would change the fetch), and `regionFetchKey` (a display hook — e.g. wiggle
includes a coarse `bpPerPx` bucket so zoom changes refetch). Detail is
chapter 04 territory; the adapter layer itself is stateless apart from parsed
headers/indexes and the chunk caches.

**The region-too-large gate** (`agent-docs/reference/REGION_TOO_LARGE.md`,
`packages/core/src/rpc/byteBudget.ts`):

- Every gated feature fetch runs `measureRegionBytes` **first**: per-region
  `getRegionByteSize` (index-only), compared against the adapter's
  `fetchSizeLimit` config slot read on the main thread (`resolvedByteLimit()`).
  Over budget → returns `{regionTooLarge: true, bytes}` instead of a payload
  (`RegionTooLargeResult`, `isRegionRefused`). Budget travels as a **call-site
  argument, never in `rpcProps()`** (so budget changes don't invalidate caches).
- Sub-floor tier: below `AUTO_FORCE_LOAD_BP` = 20 kb the budget ×
  `SUB_FLOOR_BYTE_BUDGET_FACTOR` = 2 (index bins stop shrinking below ~16 kb).
- `nextByteEstimate` sets `zoomIneffective` when halving the span yields >90%
  of the bytes — the banner drops "zoom in" advice on that axis only.
- A second, density axis (`maxFeatureDensity`, canvas displays): samples ~1 kb
  before download, refuses on features-per-pixel.
- Force-load (`forceLoadTrack`) skips both gates and survives navigation.
- Estimates survive viewport clears; dropped on chromosome nav or LOD-tier
  swap (`byteGateAdapterKey` change).

**Dedup of concurrent requests**: adapter instantiation is a cached promise;
per-track fetch dedup happens in the display fetch skeletons (in-flight guards

- `fetchGeneration` bumps, chapter 04). The BGZF worker pool shares one
  4-worker pool per context across all readers.

## 5. Data reduction in the adapter layer

- **BigWig zoom levels** are the canonical example: the bbi format's stored
  summary levels are selected by the adapter from `bpPerPx/resolution`, and the
  result is delivered as typed arrays (`RawFeatureArrays = {starts, ends,
  scores, minScores?, maxScores?, count}` — when served from a zoom level,
  min/max summaries are included and `isSummary` is set). The wiggle _display_
  then computes the autoscale domain from rendered arrays (client-side
  autoscale), so adapter-level stats fast paths are largely unused in-tree.
- **PIF coarse/fine LOD** (`PairwiseIndexedPAFAdapter`): two index tiers
  (with/without CIGAR); tier resolved main-thread-side so the fetch cache key
  covers it (`lodMode`).
- **MAF summary tier**: `BgzipTaffyAdapter` / BigMafAdapter serve block-level
  summary features at coarse zoom (the `LinearMafDisplay` swaps adapter
  configs; `byteGateAdapterPath` hook names the summary tier so the gate
  measures the file actually being read).
- **HiC** produces resolution-normalized matrices (reader-internal zoom).
- See chapter 04 for quantization done at render time (density bins,
  per-bp subpixel binning) — that is display-layer, not adapter-layer.

## 6. htsget

One adapter: `HtsgetBamAdapter` (`plugins/alignments/src/HtsgetBamAdapter/`).
Contract:

- Config: `htsgetBase` (URI of the htsget **endpoint** base) + `htsgetTrackId`.
- The adapter constructs `@gmod/bam`'s `HtsgetFile` with a `fetch` function
  scoped to the endpoint's credentials (via internet accounts);
  `@gmod/bam` v9 issues the ticket request and then fetches data blocks itself,
  applying the ticket's per-block `headers` and never sending endpoint
  credentials to a different host (spec "HTTPS data block URLs" rule 6).
- Server contract = standard htsget v1.0: `GET /reads/{id}?...` → ticket JSON
  with data-block URLs. No declared fetchSizeLimit (reports 0 = "no opinion",
  so the byte gate falls back to defaults).
- No htsget variant adapter for VCF in-tree.

## 7. Remote access / filehandles

`packages/core/src/util/io/index.ts` — `openLocation(fileLocation)`:

- `FileLocation` union (`packages/core/src/util/types/data.ts`):
  `UriLocation {uri, baseUri?}`, `LocalPathLocation {localPath}` (Node/Electron),
  `BlobLocation {blobId}` (browser-registered blobs, e.g. desktop file drops),
  `FileHandleLocation {filehandle}` (a pre-built generic-filehandle).
- Remote URIs → `RemoteFileWithRangeCache` = `@gmod/range-cache-filehandle`'s
  cached filehandle wrapping `generic-filehandle2`'s `RemoteFile`: HTTP
  **Range request** reads with an LRU chunk cache and idle sweep. All indexed
  adapters (BAM/CRAM/tabix/2bit/bbi/fai) read through this — the entire random
  access story is "range requests against plain static hosting" (S3/HTTPS;
  CORS permitting).
- **Internet accounts**: `getInternetAccount(location, pluginManager)` matches
  the URI host/prefix against configured account types (Google, HTTPBasicAuth,
  OAuth, Dropbox…) and returns an account whose `openLocation` supplies a
  credentialed fetcher. htsget uses this for the ticket request only.
- `file:` URIs on desktop are converted to localPath so indexes stay on one
  local-file code path.
- Relative URIs resolve against the config's `baseUri` **before** account
  matching (a relative URL carries no host, so matching later missed accounts).
- Plain-text reads go through `fetchAndMaybeUnzipText` (byte-progress-aware,
  transparently un-gzips); binary BGZF blocks can be inflated by
  `sharedBgzfWorkerPool()`.

## Mab notes

- **The contract is small and copyable**: `getRefNames`,
  `getFeatures(region, opts) -> stream`, `getHeader`, `getRegions`,
  `getRegionByteSize`, `getSequence`. A Rust trait with these six (features as
  an iterator or tokio stream instead of RxJS) covers 95% of the inventory.
- **Byte-estimates from indexes are the best idea in this layer**: tabix/BAI
  chunk offsets give a free pre-flight cost measurement; implement
  `bytes_for_regions(regions) -> u64` per index type and gate UI fetches with
  it. Copy the sub-floor budget (×2 below 20 kb) and `zoomIneffective`
  detection (halved span, >90% bytes) as-is.
- **Cache at two granularities**: per-context shared LRU of _decompressed
  chunks_ (bytes, ~1 GiB) across all files, plus one adapter instance per file
  config. Sharing one budget across files (not per-file) is the measured lesson.
- **Zoom levels belong in the adapter** when the format has them (BigWig
  summaries) and in the renderer when it doesn't; pass `bpPerPx` in query
  options either way, and put the resolved LOD tier in the cache key, not
  "auto".
- **Cancellation**: stop-tokens with both an await-point check and a
  synchronous probe for await-free loops; treat aborts as normal control flow
  (superseded fetches), not errors.
- **Worker output should be plain data**: JBrowse ships typed arrays with
  explicit transfer lists and 0-based half-open absolute coordinates. For a
  Rust/WASM boundary, this maps directly to `Vec<u32>`/typed-array views over
  a transferred buffer; fix the coordinate convention once (0-based half-open)
  and never convert downstream.
- **RefName namespaces**: file-internal names, assembly canonical names, and
  aliases are three vocabularies; resolve to canonical _before_ dispatching a
  query, and hand the adapter file-native names only.
- Worker pool sizing (cores−1, capped 5) and BGZF multi-worker inflate
  (~1.4–2× on decompress-bound workloads) are reasonable starting constants.
- Ambiguities to verify when porting: exact `regionFetchKey` bucketing per
  display (chapter 04), CSI vs TBI index selection in `openTabixIndexFilehandle`
  (a real historical bug source — encode the pairing in the type), and whether
  the "no size bound on adapter cache + 3-min chunk sweeper" memory profile is
  acceptable for a long-lived desktop app.
