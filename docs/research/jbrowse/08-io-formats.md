# 08 — I/O formats, text search, and config/export tooling

TL;DR: JBrowse 2 has no central format registry — adapters are plugins that register
`AdapterType`s, and the _only_ single source of truth for "what file is this" is
`packages/add-track-core/src/formats.ts` (one ordered regex table, consumed by both the
browser's `Core-guessAdapterForLocation` extension point and the CLI's `add-track`).
Everything is 0-based half-open internally; per-format index conventions (BAI/CSI/TBI/
CRAI/fai/gzi/.tai) are explicit config slots. Text search is a separate adapter family
(`BaseTextSearchAdapter`) whose dominant format is "trix" (`@gmod/trix` `.ix`/`.ixx`).

## 1. Format / adapter catalog

Adapter classes are registered via `AdapterType` (`packages/core/src/pluggableElementTypes/AdapterType.ts`),
which carries: `name`, lazy `getAdapterClass`, `configSchema`, `adapterCapabilities`
(capability strings like `getFeatures`, `getRefNames`, `getRegions`, `getHeader`),
`adapterMetadata` (incl. `alsoReads: RegExp` — alternate filenames the guesser does NOT
claim, purely an Add-track UI hint), optional `normalizeSnapshot`, and `locationKey`
(the config slot holding the primary file URI, used by indexers and import forms).

"Queryable" = random access by genomic region without streaming the whole file.
All coordinates below are **0-based half-open `[start, end)`** once inside JBrowse;
the "source" column notes where the on-disk convention differs.

### Alignments (models: chapter 06; adapter internals: chapter 05)

| Format         | Adapter                                   | Package/path                                                  | Index                                                                         | Queryable     | Source coords                           | Notes                                                                                                                                                                                               |
| -------------- | ----------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------------------------------------- | ------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| BAM            | `BamAdapter`                              | `plugins/alignments/src/BamAdapter/BamAdapter.ts`             | BAI (or CSI; `.csi` auto-detected from index filename via `resolveIndexType`) | yes           | BAM is 0-based half-open — direct match | Config: `bamLocation` + `index: { location, indexType: BAI\|CSI }`. `BamSlightlyLazyFeature` defers tag decoding                                                                                    |
| CRAM           | `CramAdapter`                             | `plugins/alignments/src/CramAdapter/CramAdapter.ts`           | CRAI                                                                          | yes           | 1-based on disk → converted             | Config: `cramLocation` + `craiLocation` (flat sidecar, not nested `index:`)                                                                                                                         |
| SAM (plain/gz) | `SamAdapter`                              | `plugins/alignments/src/SamAdapter/SamAdapter.ts`             | none                                                                          | no (streamed) | 1-based `POS` → 0-based                 | `BaseSamAdapter` shared with htsget; `parseSam.ts`                                                                                                                                                  |
| htsget BAM     | `HtsgetBamAdapter` (extends `BamAdapter`) | `plugins/alignments/src/HtsgetBamAdapter/HtsgetBamAdapter.ts` | server-side ranges                                                            | yes           | n/a                                     | Config: `htsgetBase` (must be UriLocation), `htsgetTrackId`. Ticket request only; data-block URLs fetched per the ticket's `headers` (spec rule 6 — endpoint credentials never sent to block hosts) |

### Variants (chapter 06)

| Format                         | Adapter                | Path                                                      | Index         | Queryable | Notes                                                   |
| ------------------------------ | ---------------------- | --------------------------------------------------------- | ------------- | --------- | ------------------------------------------------------- |
| VCF (plain/gz)                 | `VcfAdapter`           | `plugins/variants/src/VcfAdapter/VcfAdapter.ts`           | none          | no        | VCF is 1-based on disk; converted to 0-based internally |
| VCF bgzip                      | `VcfTabixAdapter`      | `plugins/variants/src/VcfTabixAdapter/VcfTabixAdapter.ts` | TBI/CSI       | yes       | `vcfGzLocation` + tabix index                           |
| VCF sharded                    | `SplitVcfTabixAdapter` | `plugins/variants/src/SplitVcfTabixAdapter/`              | per-shard TBI | yes       | Multi-file "split" VCF (aggregated per-sample shards)   |
| Plink LD (.ld/.vcor, plain/gz) | `PlinkLDAdapter`       | `plugins/variants/src/PlinkLDAdapter/PlinkLDAdapter.ts`   | none          | no        | LD matrix rows                                          |
| Plink LD bgzip                 | `PlinkLDTabixAdapter`  | same dir, `PlinkLDTabixAdapter.ts`                        | TBI           | yes       |                                                         |

### Features / annotations (chapter 02)

| Format                                   | Adapter                                    | Path                                                              | Index | Queryable | Notes                                                                                                          |
| ---------------------------------------- | ------------------------------------------ | ----------------------------------------------------------------- | ----- | --------- | -------------------------------------------------------------------------------------------------------------- |
| GFF3 plain/gz                            | `Gff3Adapter`                              | `plugins/gff3/src/Gff3Adapter/Gff3Adapter.ts`                     | none  | no        |                                                                                                                |
| GFF3 bgzip                               | `Gff3TabixAdapter`                         | `plugins/gff3/src/Gff3TabixAdapter/Gff3TabixAdapter.ts`           | TBI   | yes       |                                                                                                                |
| GTF plain/gz                             | `GtfAdapter`                               | `plugins/gtf/src/GtfAdapter/GtfAdapter.ts`                        | none  | no        |                                                                                                                |
| GTF bgzip                                | `GtfTabixAdapter`                          | `plugins/gtf/src/GtfTabixAdapter/GtfTabixAdapter.ts`              | TBI   | yes       |                                                                                                                |
| BED plain/gz                             | `BedAdapter` / `BedTabixAdapter`           | `plugins/bed/src/BedAdapter/`, `plugins/bed/src/BedTabixAdapter/` | —/TBI | —/yes     | `BedTabixAdapter` also reads `.bedmethyl.gz` (guessed as `MultiQuantitativeTrack`)                             |
| BedGraph (.bg) plain/gz                  | `BedGraphAdapter` / `BedGraphTabixAdapter` | `plugins/bed/src/BedGraphAdapter/`, `.../BedGraphTabixAdapter/`   | —/TBI | —/yes     | Quantitative                                                                                                   |
| BEDPE (plain/gz)                         | `BedpeAdapter`                             | `plugins/bed/src/BedpeAdapter/BedpeAdapter.ts`                    | none  | no        | Pairwise/structural; no tabix variant in-tree                                                                  |
| narrowPeak                               | — none —                                   |                                                                   |       |           | No dedicated adapter; readable as generic BED-family if tabixed, but no guess entry                            |
| STAR-Fusion TSV                          | `StarFusionAdapter`                        | `plugins/bed/src/StarFusionAdapter/StarFusionAdapter.ts`          | none  | no        | Identified only by `star-fusion`/`fusion_predictions` in filename + `.tsv`                                     |
| GWAS (tabix `*.txt.gz`, Pan-UKBB layout) | `GWASAdapter`                              | `plugins/gwas/src/GWASAdapter/GWASAdapter.ts`                     | TBI   | yes       | Deliberately NOT guessed from `.bed.gz` (would need column sniffing); score transforms in `scoreTransforms.ts` |
| Generic tabix                            | (any `*TabixAdapter`)                      |                                                                   | TBI   | yes       | Format-agnostic querying lives in htslib-style block logic per adapter                                         |

### Quantitative (chapter 04 render)

| Format               | Adapter                   | Path                                                          | Index               | Queryable | Notes                                                                |
| -------------------- | ------------------------- | ------------------------------------------------------------- | ------------------- | --------- | -------------------------------------------------------------------- |
| BigWig (.bw/.bigwig) | `BigWigAdapter`           | `plugins/wiggle/src/BigWigAdapter/BigWigAdapter.ts`           | intrinsic (B+ tree) | yes       | Self-indexing container                                              |
| Multi-wiggle         | `MultiWiggleAdapter`      | `plugins/wiggle/src/MultiWiggleAdapter/MultiWiggleAdapter.ts` | via sub-adapters    | yes       | Config lists multiple sources each with its own adapter              |
| wig/bedGraph         | `BedGraphAdapter` (above) |                                                               | —                   | no        | No classic `.wig` adapter; wiggle rendering handled at display layer |

### Sequence / reference (chapter 01 assembly)

| Format                    | Adapter                                                             | Path                                                                  | Index               | Queryable       | Notes                                                                                            |
| ------------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------- | ------------------- | --------------- | ------------------------------------------------------------------------------------------------ |
| FASTA + .fai              | `IndexedFastaAdapter`                                               | `plugins/sequence/src/IndexedFastaAdapter/IndexedFastaAdapter.ts`     | FAI                 | yes             | FAI: 1-based byte offsets on disk, resolved to 0-based internally                                |
| bgzip FASTA               | `BgzipFastaAdapter`                                                 | `plugins/sequence/src/BgzipFastaAdapter/BgzipFastaAdapter.ts`         | FAI + GZI           | yes             | Two sidecars (`faiLocation`, `gziLocation`)                                                      |
| Unindexed FASTA           | `UnindexedFastaAdapter`                                             | `plugins/sequence/src/UnindexedFastaAdapter/UnindexedFastaAdapter.ts` | none                | no (whole-file) |                                                                                                  |
| 2bit                      | `TwoBitAdapter`                                                     | `plugins/sequence/src/TwoBitAdapter/TwoBitAdapter.ts`                 | intrinsic           | yes             |                                                                                                  |
| chrom.sizes (.sizes)      | `ChromSizesAdapter`                                                 | `plugins/sequence/src/ChromSizesAdapter/ChromSizesAdapter.ts`         | none                | regions only    | Guesser marks `.sizes` **unsupported** for tracks — it's an assembly/regions format, not a track |
| `trackData.json` (NCList) | `NCListAdapter`                                                     | legacy (guessed from `trackData.jsonz?`)                              | intrinsic           | yes             | JBrowse 1 stores                                                                                 |
| Motifs in reference       | `MotifListAdapter`, `SequenceSearchAdapter`, `ReferenceScanAdapter` | `plugins/sequence/src/`                                               | none                | scan            | Regex/PWM scan over the reference sequence adapter — computed, not stored                        |
| CRISPR guides             | `CrisprGuideAdapter`                                                | `plugins/sequence/src/CrisprGuideAdapter/`                            | scan over reference | computed        |                                                                                                  |

### MAF / multiple alignment (`plugins/maf`; chapter refs: MAF cross-view notes)

| Format           | Adapter             | Path                                                     | Index            | Queryable | Notes                                                                         |
| ---------------- | ------------------- | -------------------------------------------------------- | ---------------- | --------- | ----------------------------------------------------------------------------- |
| MAF bgzip + .tai | `BgzipMafAdapter`   | `plugins/maf/src/BgzipMafAdapter/BgzipMafAdapter.ts`     | Taffy `.tai`     | yes       | `.tai` auto-resolved from `uri` shorthand (`${uri}.tai`); provides zoom tiers |
| MAF tabix        | `MafTabixAdapter`   | `plugins/maf/src/MafTabixAdapter/MafTabixAdapter.ts`     | TBI              | yes       | Locus-sliced MAF                                                              |
| BigMaf (.bb)     | `BigMafAdapter`     | `plugins/maf/src/BigMafAdapter/BigMafAdapter.ts`         | intrinsic BigBed | yes       |                                                                               |
| Taffy pair       | `BgzipTaffyAdapter` | `plugins/maf/src/BgzipTaffyAdapter/BgzipTaffyAdapter.ts` | `.tai`           | yes       | Same `.tai` format as BgzipMaf; Cactus/taffy tooling writes it                |

### Comparative / synteny (`plugins/comparative-adapters`, models: chapter 06)

| Format                                      | Adapter                                                                       | Index | Queryable      | Notes                                                                                                                   |
| ------------------------------------------- | ----------------------------------------------------------------------------- | ----- | -------------- | ----------------------------------------------------------------------------------------------------------------------- |
| PAF (.paf/.paf.gz)                          | `PAFAdapter`                                                                  | none  | no (in-memory) | Pairwise alignments                                                                                                     |
| PIF (.pif.gz + .tbi)                        | `PairwiseIndexedPAFAdapter`                                                   | TBI   | yes            | "PIF" = pairwise indexed PAF; `jbrowse make-pif` builds it                                                              |
| All-vs-all PAF                              | `AllVsAllPAFAdapter`                                                          | none  | in-memory      | Same extension as PAF — indistinguishable by name, chosen via adapter hint; PanSN naming maps query names to assemblies |
| All-vs-all PIF                              | `AllVsAllIndexedPAFAdapter`                                                   | TBI   | yes            | Same indistinguishability                                                                                               |
| mashmap .out                                | `MashMapAdapter`                                                              | none  | in-memory      |                                                                                                                         |
| BLAST tabular                               | `BlastTabularAdapter`                                                         | none  | in-memory      | No extension guess (format ambiguous)                                                                                   |
| chain (.chain)                              | `ChainAdapter`                                                                | none  | in-memory      |                                                                                                                         |
| delta (.delta, MUMmer)                      | `DeltaAdapter`                                                                | none  | in-memory      |                                                                                                                         |
| MCScan anchors (.anchors / .anchors.simple) | `MCScanAnchorsAdapter` / `MCScanSimpleAnchorsAdapter` / `MCScanBlocksAdapter` | none  | in-memory      | `.anchors.simple.gz` must be guessed before `.anchors.gz`                                                               |

All comparative adapters extend `ComparativeAdapterBase` / `PairwiseAdapterBase`
(`plugins/comparative-adapters/src/`).

### Misc / config-sourced

| Adapter                                                                      | Path                                                    | Notes                                                                                                                                      |
| ---------------------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `FromConfigAdapter`, `FromConfigRegionsAdapter`, `FromConfigSequenceAdapter` | `plugins/config/src/FromConfig*/`                       | Features/regions/sequence held inline in config JSON — the "no file" formats                                                               |
| `RefNameAliasAdapter`, `NcbiSequenceReportAliasAdapter`                      | `plugins/config/src/`                                   | Assembly alias resolution (chapter 01)                                                                                                     |
| `SPARQLAdapter`                                                              | (guessed from bare `sparql` pseudo-name)                | Endpoint-based features                                                                                                                    |
| `JBrowse1TextSearchAdapter`                                                  | `plugins/legacy-jbrowse/src/JBrowse1TextSearchAdapter/` | Text search against JBrowse 1 `names/` HTTP index (see §3)                                                                                 |
| `HicAdapter`                                                                 | `plugins/hic/src/HicAdapter/HicAdapter.ts`              | `.hic` (Juicer) — intrinsic index, `getRefNames`/`getHeader` from file; **no** `getFeatures` (display queries blocks directly; chapter 04) |

### Spreadsheet-view import adapters (not track adapters)

`plugins/spreadsheet-view/src/SpreadsheetView/importAdapters/` — CSV/TSV/VCF/BED/BEDPE/STAR-Fusion
**table import** into the spreadsheet view (export to track via `AddTrack` workflows).
`VcfImport.ts`, `BedImport.ts`, `BedpeImport.ts`, `STARFusionImport.ts`. These are the
answer for "a tabular file becomes a track": it goes spreadsheet → add-track, not
adapter-guess.

**Ambiguity noted:** "narrowPeak" and classic "wig" have no first-class adapter or guess
entry in this codebase; treat them as BED-family/tabix generics.

## 2. Format auto-detection: dropped file → track config

The whole mechanism is two extension points plus one table:

- `Core-guessAdapterForLocation(file, indexLocation, adapterHint)` — returns an adapter
  config snapshot (`{ type: 'BamAdapter', bamLocation: {...}, index: {...} }`) or undefined.
- `Core-guessTrackTypeForLocation(adapterType, file)` — returns track type or undefined
  (default `'FeatureTrack'`; `packages/core/src/util/tracks.ts` `guessTrackType`).
  Track types per adapter come from the generated map in
  `packages/add-track-core/src/trackTypes.generated.ts` (`adapterTypesToTrackTypeMap`),
  cross-checked against each adapter's `#trackType` tag.

### The table: `packages/add-track-core/src/formats.ts`

`export const formats: FormatEntry[]` — **ordered list, first regex match wins** (ADR-077:
one table, two consumers). Each `FormatEntry`:

- `regex?: RegExp | RegExp[]` — matched against the **bare filename**; an array = all must
  match (multi-condition, e.g. STAR-Fusion). Absent ⇒ reachable only by naming the adapter
  type (all-vs-all PAF, BlastTabular, MCScanBlocks).
- `spec: AdapterSpec` — four kinds:
  - `{ kind: 'single', adapterType, locField }` — one location field (BigWig, .hic, PAF…).
  - `{ kind: 'indexed', adapterType, locField, suffix, indexType: 'BAI'|'TBI' }` — index nests
    at `${locField} index: { location: {uri: file+suffix}, indexType }` (BAM, all tabix).
  - `{ kind: 'sidecar', adapterType, locField, sidecars: [{field, suffix, fromIndex?}] }` —
    flat sibling location fields (CRAM `craiLocation`; FASTA `faiLocation`+`gziLocation`).
  - `{ kind: 'anchors', ... }` — single + two optional BED files (CLI `--bed1/--bed2`).
  - `{ kind: 'unsupported' }` — recognized extension, deliberately refused (`.vcf.idx`, `.sizes`).
- `trackType?` — overrides for adapters that serve two formats sharing an extension
  (`.bedmethyl.gz` → `MultiQuantitativeTrack` via `BedTabixAdapter`).

Key behaviors:

- **Order constraints**: tabix entries before plain; `.anchors.simple` before `.anchors`.
- `matchFormat(fileName, adapterHint?)` — an explicit adapter hint **always wins** over
  the filename (that is how the Add-track form opens ambiguous files).
- `resolveIndexType(indexName, fallback)` — htslib writes **CSI** instead of BAI/TBI for
  references > 512 Mb (or on request at any size); detected purely by the index filename
  ending in `.csi`.
- An entry only takes effect if its adapter type is **registered** in the running build —
  core's guess chain consults the table but the plugin providing the adapter decides.

### From file to config

- `guessTrackConf(input, pluginManager, assemblyName?)` (`packages/core/src/util/tracks.ts`)
  — headless inference: adapter guess → `trackId = "<filename>-<objectHash(adapter).slice(0,8)>"`,
  `name = filename`, `type` from track-type guesser (default `FeatureTrack`), `assemblyNames`.
  Throws when no format matches so the caller can substitute
  `generateUnsupportedTrackConf` / `generateUnknownTrackConf` (a `FeatureTrack` named
  "... (Unsupported)" with a hash-derived trackId — the track shows an error banner).
- **Loose track configs**: a config snapshot of form `{ trackId?, uri, ... }` with no
  `adapter` is expanded by `expandLooseTrackConfig` (same file) on **every** path a track
  snapshot enters the tree — config union, root config `tracks`, `addTrackConf`,
  `showTrackGeneric`. So a JBrowse config can literally contain
  `{ "trackId": "reads", "uri": "reads.bam", "assemblyNames": ["hg38"] }`.
- Local files: `packages/product-core/src/localFiles.ts` — blobs registered under names,
  tracks point at `BlobLocation`; `BlobFile` serves **byte ranges out of `Blob.slice()`**, so
  bgzip+tabix/BAM+BAI/BigWig work seek-based on dropped files with no server. A loose spec
  is run through `guessTrackConf` + `normalizeAdapterSnapshots`
  (`packages/product-core/src/controllerTracks.ts`).
- `packages/add-track-core` also drives the interactive Add-track form (with the
  `alsoReads` metadata offering alternates when the extension is ambiguous).

## 3. Text search

Four code units: `packages/text-indexing-core` (shared indexer), `packages/text-indexing`
(quality-of-life lib), `plugins/text-indexing` (RPC method `TextIndexRpcMethod` so the
desktop app can index in a worker), `plugins/trix` (the search-side adapter), plus core
`packages/core/src/TextSearch/TextSearchManager.ts`.

### What gets indexed

`packages/text-indexing-core/src/util.ts` `indexableAdapters` — exactly six adapter types:

| Adapter                            | locationKey                     | format |
| ---------------------------------- | ------------------------------- | ------ |
| `Gff3Adapter` / `Gff3TabixAdapter` | `gffLocation` / `gffGzLocation` | gff3   |
| `GtfAdapter` / `GtfTabixAdapter`   | `gtfLocation` / `gtfGzLocation` | gtf    |
| `VcfAdapter` / `VcfTabixAdapter`   | `vcfLocation` / `vcfGzLocation` | vcf    |

Defaults (`same file`): `defaultAttributesToIndex = ['Name','ID','symbol']`;
`defaultFeatureTypesToExclude = ['exon','CDS']`. Per-track overrides live in the track
config under `textSearching: { indexingAttributes, indexingFeatureTypesToExclude,
indexingFeatureTypesToInclude }`, plus `metadata.skipTextIndex` to skip a track.

`indexFiles({tracks, attributesToIndex, outDir, ...})` (`packages/text-indexing-core/src/indexFiles.ts`)
streams records per track, dispatching to `indexGff3` / `indexGtf` / `indexVcf` (also in
`text-indexing-core/src/types/`), with progress/abort callbacks.

### On-disk/remote index format: trix

`packages/text-indexing-core/src/trixPaths.ts` — per index `name`, three artifacts under
`trix/` (sanitized for Windows; reserved device names like `NUL` handled):

- `<name>.ix` — the sorted term index (format consumed by `@gmod/trix`; newline-separated
  records `term\tdetail`).
- `<name>.ixx` — the prefix lookup index over `.ix` (maps term prefixes to byte offsets in
  `.ix`; prefix length auto-computed, overridable via `prefixSize` — increase when IDs share
  long common prefixes like `Z000000001`).
- `<name>_meta.json` — provenance/config for the index.

The other search format is JBrowse 1's legacy names index: `JBrowse1TextSearchAdapter`
(`plugins/legacy-jbrowse/.../JBrowse1TextSearchAdapter.ts`) with slots `namesIndexLocation`
(a directory URI), `tracks`, `assemblies`.

### Query flow

- **Adapters**: `TrixTextSearchAdapter` (`plugins/trix/src/TrixTextSearchAdapter/TrixTextSearchAdapter.ts`)
  config slots `ixFilePath` + `ixxFilePath`; loads `@gmod/trix` with a 1500-entry buffer and
  serves `searchIndex({ queryString, searchType })` where `searchType` ∈
  **`prefix` | `exact` | `full`** (full = substring/fuzzy-ish). Results are `BaseResult`s
  (`packages/core/src/TextSearch/BaseResults.ts`) carrying `label`, `displayString`,
  `matchedObject` (feature/locus/refSeq), and a contextual snippet
  (`snippetAround`, ±15 chars around the match).
- **Manager**: `TextSearchManager` (`packages/core/src/TextSearch/TextSearchManager.ts`) —
  caches adapter instances (QuickLRU, 15); `relevantAdapters(assemblyName)` collects
  ① root-config `aggregateTextSearchAdapters[]` (site-wide indexes) and ② per-track
  `textSearchAdapter` configs whose `searchableAssemblyNames` include the (alias-canonicalized)
  assembly; runs all in parallel with `Promise.allSettled` — **a broken index is logged, not
  fatal** (aborts are silent, one query per keystroke supersedes the last); refName results
  are merged in afterwards by the caller.
- Config wiring: `createTextSearchConf`/`generateMeta` helpers wire `.ix/.ixx` URIs and a
  `textSearchAdapterId` into track configs (also used by desktop's in-app indexer,
  `products/jbrowse-desktop/src/indexJobsModel.ts`, which runs indexing jobs through
  `@jbrowse/text-indexing` in a worker and patches the trix adapter confs into tracks).

## 4. Exporters / output products

- **SVG export (in-browser)** — `SvgChrome` gate + `computeSvgReady`/`awaitSvgReady`;
  every canvas display must supply a Canvas2D draw fn and SVG export runs that fn, never
  the GPU shader path. Details: chapter `04-track-display-render.md` and
  `agent-docs/reference/SVG_EXPORT.md`. Comparative/circular/breakpoint views expose
  per-view `renderToSvg` functions (imported by jbrowse-img below).
- **`products/jbrowse-img`** — headless renderer: builds a session + views from args/config
  files/hubs, calls the `renderToSvg` implementations (linear, dotplot, synteny, circular,
  breakpoint — `products/jbrowse-img/src/renderRegion.ts`) and writes **SVG (stdout by
  default), PNG, or PDF by output-file extension**; batch mode has an explicit `--format`
  enum. Supports breakpoint/BEDPE-junction/comparative loops, track/display option
  overrides (`applyTrackOpts.ts`, `applyDisplayOpts.ts`).
- **`products/jbrowse-capture`** — Playwright-driven **screenshot** of a live JBrowse
  session URL (`page.screenshot`, full-page option) after waiting for
  settled/rendered gates (`waits.ts`, `pendingDisplays.ts`, `sessionGate.ts`) — i.e. it
  captures the real app (GPU path included), unlike jbrowse-img's direct SVG render.
- **Saved track formats (alignments)** — `plugins/alignments/src/saveTrackFormats/sam.ts`:
  `stringifySAM` writes a SAM document from features (header `@HD`/`@SQ` from the assembly
  manager's regions, `@PG ID:jbrowse`; per record **`start + 1`** and `nextPos + 1` — the
  0-based internal values are re-based to SAM's 1-based `POS`). Wired via
  `AlignmentsTrack`'s export menu (`saveTrackFormats` capability). Optional tags are not yet
  emitted (noted in source).

## 5. jbrowse-cli command inventory

`products/jbrowse-cli` — dispatch registry is a single list in `products/jbrowse-cli/src/index.ts`
(commands loaded lazily on dispatch, for startup cost):

| Command                 | Purpose / config semantics                                                                                                                                                                                                                                                                  |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `create`                | Download & install latest JBrowse 2 release                                                                                                                                                                                                                                                 |
| `add-assembly`          | Add an assembly stanza (`assemblies[]` in config.json); FASTA + index wiring; alias support (`products/jbrowse-cli/src/commands/add-assembly/`)                                                                                                                                             |
| `add-track`             | Add a track — reuses `@jbrowse/add-track-core` `formats.ts` + `matchFormat`/`trackTypeForAdapter`, so CLI inference is byte-identical to the browser's; supports explicit adapter hint, index override, CSI detection                                                                       |
| `add-track-json`        | Insert a raw JSON track config hunk verbatim                                                                                                                                                                                                                                                |
| `add-connection`        | Add a `connections[]` entry (JBrowse 1 / htsget / other data source refs)                                                                                                                                                                                                                   |
| `remove-track`          | Delete a track stanza by trackId                                                                                                                                                                                                                                                            |
| `set-default-session`   | Write a default session (views + shown tracks) into config                                                                                                                                                                                                                                  |
| `text-index`            | Build trix indexes — three modes: `aggregate` (one index per assembly), `per-track`, `file-list` (`--file/--fileId` for tracks not in config); computes ixx prefix size, writes `.ix/.ixx/_meta.json`, and **patches `textSearchAdapter` configs into the config file** (`config-utils.ts`) |
| `make-pif`              | bgzip+tabix a PAF into PIF (pairwise indexed PAF)                                                                                                                                                                                                                                           |
| `sort-gff` / `sort-bed` | Pre-sort for tabix (`sort -k1,1 -k4,4n` / `-k1,1 -k2,2n`, headers on top)                                                                                                                                                                                                                   |
| `validate`              | Config linter — includes checks JBrowse accepts silently (e.g. display-node slot-in-wrong-place), and uses `Core-guessAdapterForLocation` to check loose track configs resolve                                                                                                              |
| `admin-server`          | Small HTTP admin server for runtime config editing                                                                                                                                                                                                                                          |
| `upgrade`               | Upgrade installed JBrowse 2                                                                                                                                                                                                                                                                 |

All config edits are JSON in place (config.json / session spec), preserving formatting
reasonably; the CLI runs headless Node (no plugin system beyond registered core+format
plugins, which is why the shared table lives in `add-track-core`).

## 6. jbrowse-desktop (brief)

Electron product; config = same JBrowse config model, with: an in-app **text-indexing job
queue** (`products/jbrowse-desktop/src/indexJobsModel.ts`) that indexes local tracks via
`@jbrowse/text-indexing` in a worker and attaches trix search adapters to tracks;
`corePlugins.ts` fixed plugin set; file drag/open handled by the shared local-file/blob
mechanism (§2) — no server needed.

## Mab notes

- **One ordered "formats" table is the right shape.** `add-track-core/formats.ts` is ~30
  entries mapping filename regex → (adapter, index spec, track type), shared by GUI, CLI,
  and headless validation. In Rust: a static `&[FormatEntry]` with `MatchKind`
  (single/indexed/sidecar/unsupported), and let "adapter registered" gate each entry.
- **Make index sidecars explicit config, with defaults from filename** (`file.gz` →
  `file.gz.tai`, `.bam` → `.bai`/`.csi` by filename). CSI-vs-BAI/TBI disambiguation by
  index filename suffix is all JBrowse does — cheap and correct; support CSI from day one
  (>512 Mb references).
- **Loose configs** (`{uri: ...}` → full track) are a big UX win and trivial to implement:
  one `expand_loose_track(spec) -> Result<TrackConf>` on every config-ingestion path,
  plus fallback "Unsupported" placeholder tracks instead of hard errors.
- **Separate the search adapter family from data adapters.** A `TextSearchAdapter` trait
  (`search(query, SearchType{Prefix,Exact,Full}) -> Vec<SearchResult>`) with trix as the
  default format is a small, high-value port; the `.ix/.ixx` format is simple enough to
  write natively (sorted `term\tdetail` + prefix-offset sidecar) and indexes build from
  GFF3/GTF/VCF attributes only (Name/ID/symbol defaults).
- **Byte-range seeking over Blobs is why local files need no server.** Rust equivalent is
  free: `Read+Seek` over any backend; design adapters against a `Location` enum
  (Url/Path/Bytes) with range reads, like `openLocation`.
- **Coordinate discipline:** pick 0-based half-open internally everywhere; each parser
  converts at the boundary (VCF/SAM/GFF are 1-based; BED/BAM already 0-based). The only
  re-conversion back is export (SAM `POS = start+1`).
- **Comparative formats (PAF/PIF/chain/delta/mashmap/BLAST)** are all "parse whole file into
  memory, no random access" — fine for a first Rust implementation; the indexed exception
  (PIF+tabix) shows the upgrade path when all-vs-all sets get large.
- **Deliberate refusals matter**: `.sizes` and `.vcf.idx` are recognized-and-refused so
  users get a clear message instead of a misparse; and GWAS-tabix is not guessed from
  `.bed.gz` because extension sniffing cannot distinguish them — a lesson: when two formats
  share an extension, require an explicit adapter hint rather than guessing columns.
- **Failure isolation in search**: `allSettled` semantics — one dead index never blocks the
  others or the refName fallback. Replicate.
