# 01 — The Assembly (Reference Genome) Model

Chapter of _JBrowse 2 v4.3.0 (83ac4507cf) — an agent-friendly architectural
reference for the Mab reimplementation_. Scope:
`packages/core/src/assemblyManager/*`, the sequence adapters in
`plugins/sequence`, the alias adapters in `plugins/config`, and the `Region`
type in `packages/core/src/util/types/`.

**TL;DR** An assembly is (1) a config record keyed by a unique `name` with
`aliases`, (2) a _ReferenceSequenceTrack_ whose adapter defines both the
sequence bytes and the canonical contig list, and (3) optional sidecar files —
alias table, cytobands, genetic-code TSV. Everything else at runtime
(regions, refName alias maps, per-adapter refName maps, mismatch diagnostics)
is derived, cached in volatiles, and loaded lazily on first observation.
Coordinates are 0-based half-open `[start, end)` throughout.

---

## 1. Config schema: `BaseAssembly`

`packages/core/src/assemblyManager/assemblyConfigSchema.ts`. One schema,
`ConfigurationSchema('BaseAssembly', …, { explicitIdentifier: 'name' })` —
**the `name` slot _is_ the primary key**; there is no separate id. A
longer human-readable label is `displayName`.

| Slot                   | Type                                        | Default       | Notes                                                                                                    |
| ---------------------- | ------------------------------------------- | ------------- | -------------------------------------------------------------------------------------------------------- |
| `name`                 | string (identifier)                         | —             | machine-readable id, e.g. `hg38`                                                                         |
| `aliases`              | `stringArray`                               | `[]`          | other names for the same assembly, e.g. `['GRCh38']`; used by `assemblyManager.has` / `assemblyNameMap`  |
| `sequence`             | full _ReferenceSequenceTrack_ config schema | —             | the only mandatory payload; its adapter supplies contig list + sequence                                  |
| `refNameColors`        | `stringArray`                               | `[]`          | per-contig colors, cycled; empty ⇒ `defaultRefNameColors` (UCSC palette, 26 entries, `refNameColors.ts`) |
| `geneticCodes`         | `frozen`                                    | `{}`          | inline map refName → NCBI translation-table id, e.g. `{ chrM: 2 }`                                       |
| `geneticCodesLocation` | `fileLocation`                              | `{ uri: '' }` | optional sidecar TSV `refName<TAB>geneticCodeId` (`#` comments OK); inline map wins on conflict          |
| `refNameAliases`       | sub-schema `RefNameAliases`                 | —             | `{ adapter }`; `preProcessSnapshot` fills a `RefNameAliasAdapter` from a bare `{ uri }`                  |
| `cytobands`            | sub-schema `Cytoband`                       | —             | `{ adapter }`; shorthand fills a `CytobandAdapter` from `{ uri }`                                        |
| `displayName`          | string                                      | `''`          | e.g. "Homo sapiens (hg38)"                                                                               |

No slots for `species`/`taxonId` on the base schema (those appear on product
shims, not core). Cytobands are the only "ideogram/banding" data source; there
is no other chromosome-band model.

### Shorthand expansion — `expandAssemblyConfigShorthand.ts`

`expandAssemblyShorthand(snap, pluginManager)` is the schema's
`preProcessSnapshot` (lifted out so callers that need canonical locations
before MST builds the tree can run it themselves; idempotent). Three levels:

1. **Flat**: `{ name: 'hg38', uri: 'genome.fa.gz' }` → synthesizes
   `sequence: { adapter: { uri } }`. `baseUri` (stamped by `addRelativeUris`
   for relative/hub configs) rides down onto the adapter.
2. **Adapter-type guessing**: `sequence.adapter.type` omitted but `uri` present
   → `expandAssemblySequenceAdapter` calls the
   `Core-guessAdapterForLocation` extension point (`packages/core/src/util/tracks.ts`)
   — the same first-match-wins chain the Add-track flow uses — which maps
   `.fa.gz` → `BgzipFastaAdapter`, `.fa` → `IndexedFastaAdapter`,
   `.2bit` → `TwoBitAdapter`, and derives the `.fai`/`.gzi` sibling index
   locations. Explicit adapter fields are spread on top (they win).
   A `uri` matching no guesser is left untouched so the real error surfaces
   downstream.
3. **Track boilerplate**: `sequence.type`/`sequence.trackId` omitted → filled
   with `'ReferenceSequenceTrack'` and `` `${name}-ReferenceSequenceTrack` ``.

Minimal legal config is therefore `{ name: 'hg38', uri: 'genome.fa.gz' }`.

---

## 2. The `Assembly` runtime model

`packages/core/src/assemblyManager/assembly.ts` (`assemblyFactory`).

### MST field inventory (these _are_ the data structures)

- `#property configuration` — `types.safeReference(assemblyConfigType)`; may
  resolve to `undefined` when the backing config was removed (orphan detection).
- `#volatile` (the loaded/derived state, none of it serialized):
  - `error: unknown` — load failure, mirrored for reactive consumers only.
  - `loadingP?: Promise<void>` — the in-flight/idempotent load promise; cleared
    on failure so the next call retries.
  - `adapterLoads: QuickLRU<string, Promise<RefNameAliases>>` (maxSize 1000) —
    per-instance cache of **per-adapter refName maps**, keyed by
    `adapterConfigCacheKey(adapterConf)`. Instance-scoped, not closure-scoped,
    because the same adapter config queried under two assemblies (comparative
    views) resolves differently. Entries evicted on failure so a retry works.
  - `volatileRegions?: BasicRegion[]` — the contig list (see §3).
  - `refNameAliases?: RefNameAliases` — `Record<aliasOrCanonical, canonical>`
    (see §4). Presence of this field _defines_ `initialized`.
  - `canonicalToSeqAdapterRefNames?: Record<string,string>` — canonical name →
    the name as spelled in the FASTA (differs only with `override:true`
    aliases).
  - `cytobands?: Feature[]` — cytoband file as SimpleFeatures.
  - `loadedGeneticCodes?: Record<string, number>` — from the sidecar TSV.
  - `lowerCaseRefNameAliases?: RefNameAliases` — lowercase-keyed second index.
  - `statusMessage?: string`, `statusProgress?: number (0–1)`,
    `statusSource?: string` — aggregate progress from the four parallel loads.
  - `refNameMismatches: Map<string, RefNameMismatch>` — adapter cache key →
    empty-intersection verdict (`refNameMismatch.ts`). Written by **replacing**
    the Map (a Map inside a volatile is one observable; `.set()` would leave
    readers stale).
- Key getters/methods: `name`, `aliases`, `displayName` (falls back to name),
  `allAliases` = `[name, ...aliases]`, `hasName`, `refNameColors`,
  `initialized` (`!!refNameAliases`), `regions`, `refNames` (regions' refNames),
  `allRefNames` (alias-map keys — superset of canonical names),
  `namesByCanonicalRefName` (inverted grouping, canonical first),
  `refNameToIndex` (memoized first-index map for O(1) color/region lookup),
  `getRegionForRefName`, `getCanonicalRefName`, `getCanonicalRefName2` (total
  variant, §4), `getSeqAdapterRefName`, `isValidRefName`, `getAliasesForRefName`,
  `getGeneticCodeId`, `getRefNameMapForAdapter(adapterConf, opts)` (per-adapter
  map, memoized, requires `sessionId`), `getRefNameMismatch(adapterCacheKey)`,
  `load()`, `rpcManager` (reached via parent chain).

### Lifecycle

- **Lazy init**: `afterAttach` installs `onBecomeObserved` on both
  `volatileRegions` and `refNameAliases`; the first reactive read of either
  kicks `load()` (fire-and-forget, idempotent). Nothing loads until something
  observes it.
- **`load()`** — memoizes `loadPre()` in `loadingP`; clears a prior `error`
  first; on failure clears `loadingP`, sets `error`, logs, and rethrows (the
  rejection is the authoritative signal for awaiters; `error` is for the UI).
- **`loadPre()`** runs **four independent loads in one `Promise.all`**, all
  with a shared status fan-out:
  1. `getAssemblyRegions({ config: conf.sequence.adapter })` —
     `assemblyAdapters.ts` instantiates the adapter **on the main thread**
     (no RPC) and calls `getRegions()`.
  2. `getRefNameAliases({ config: conf.refNameAliases?.adapter })`.
  3. `getCytobands({ config: conf.cytobands?.adapter })`.
  4. `getGeneticCodesFromFile({ location: conf.geneticCodesLocation })`.
     Region refNames are validated with `checkRefName` (SAM-spec grammar, §4),
     then `buildRefNameMaps(regions, aliases)` builds the three maps, then
     `setLoaded` applies everything in **one transaction** (regions' refNames are
     rewritten to canonical names at this point, with `assemblyName` stamped).
- **How views wait**: `assemblyManager.waitForAssembly(name)` →
  `requireAssembly(name)`; both `await assembly.load()` — the promise resolves
  only after `setLoaded`. `assemblyManager.loadingAssembly(names)` returns the
  first not-yet-`initialized` assembly so a spinner can show its
  `statusMessage`.

---

## 3. `AssemblyManager` and the `Region` type

### AssemblyManager

`packages/core/src/assemblyManager/assemblyManager.ts`.

- `assemblies: Assembly[]` — models auto-synced from configs by an
  `afterAttach` autorun over `assemblyList` =
  `jbrowse.assemblies ∪ session.sessionAssemblies ∪ session.temporaryAssemblies`
  (three config homes: site-wide, session-added, temporary). Orphaned models
  (config removed) are pruned.
- `assemblyNameMap`: `Record<nameOrAlias, Assembly>` — **assembly names alias
  exactly like refNames**. `getCanonicalAssemblyName`, `getDisplayName`.
- `has(name)` — true if the map has it _or_ `configuredAssemblyNames` (a Set
  built from configs, available before models are built; covers aliases).
  `get(name)` vs `has(name)` differ in side effects: `get` **reports unknown
  names** to the `Core-handleUnrecognizedAssembly` extension point (once per
  name per session) so a plugin can go supply the assembly (e.g. a hub
  connection); a handler may return a promise which `waitForAssembly` awaits.
- `waitForAssembly(name)`: get → if missing, `settleAssemblyResolution`
  (await the handler's claim, then `when(config present || no connections
  loading)`) → re-get → `await assembly.load()`. Returns `undefined` if
  unresolvable; `requireAssembly` throws instead. **No timeouts on the
  resolution path** — it waits on events, not clocks (except a 10 s grace for
  handlers that returned no promise).
- `getRefNameMapForAdapter(adapterConf, assemblyName, opts)` — the entry RPC
  plumbing uses to rename regions outbound (§4).

### Region types

`packages/core/src/util/types/mst.ts` + `.../types/data.ts`:

```ts
// MST model, MST snapshot = the plain TS type
NoAssemblyRegion = { refName: string, start: number, end: number,
                     reversed?: boolean /* default false */ }
Region           = NoAssemblyRegion & { assemblyName: string }
BasicRegion      // assembly.ts: { start, end, refName, assemblyName }
AugmentedRegion  = Region & { originalRefName?: string }
```

All coordinates are **0-based half-open** (`[start, end)`), interbase; `end` is
exclusive. `reversed` marks a region displayed in its reverse complement in the
view (comparative views can flip a region without recomputing coordinates:
`region.reversed ? region.end - bp : bp - region.start` in `Base1DUtils.ts`).
`assemblyName` is the canonical assembly name — a name crossing the RPC
boundary must already be canonical (worker has no assembly manager).

**Whole-genome region list**: only from the sequence adapter. The
`RegionsAdapter` interface (`packages/core/src/data_adapters/BaseAdapter/RegionsAdapter.ts`)
is `getRegions(opts): Promise<NoAssemblyRegion[]>`; the
`ChromSizesAdapter` shows the minimal contract — parse `name<TAB>length`
lines, emit `{ refName, start: 0, end: length }`. The assembly's
`volatileRegions` are these regions with canonical refNames + `assemblyName`
stamped, in adapter/file order (which is therefore also chromosome display
order).

**Cytobands**: `packages/core/src/data_adapters/CytobandAdapter/CytobandAdapter.ts`
reads the UCSC `cytoBand.txt` format — tab-separated
`refName, start, end, name, type`, `#` comments skipped — into `SimpleFeature`s
with fields `name`, `type`, `gieStain`. Coordinates 0-based half-open (UCSC's
file is 0-based start / 1-based end; the adapter does **no** end adjustment —
an off-by-one carried into JBrowse, worth not copying).

---

## 4. RefName aliasing and canonicalization

### The maps — `refNameMaps.ts`

Inputs: the sequence adapter's regions (ground truth) and the alias
collection, `Alias[] = { refName, aliases: string[], override?: boolean }`.

`buildRefNameMaps(regions, aliases) → { refNameAliases,
lowerCaseRefNameAliases, canonicalToSeqAdapterRefNames }`:

- `override` is **true by default/unset**: the alias file's `refName` column
  becomes canonical, and its `aliases[]` all map to it.
- `override: false`: the canonical name is whichever of the row's aliases
  matches a FASTA contig name (`aliases.find(a => fastaRefNames.has(a))`),
  i.e. the _sequence file_ stays canonical.
- Every canonical name is identity-mapped into `refNameAliases`
  (`refNameAliases[canonical] ??= canonical`); where the canonical name differs
  from the sequence adapter's name, `canonicalToSeqAdapterRefNames[canonical]
  = adapterName` records the FASTA spelling.
- `lowerCaseRefNameAliases` is a lowercase-keyed second index of the same map —
  this is how `chr1` resolves case-insensitively.
- Every alias string is validated by `checkRefName`: the SAM v1 §1.2.1
  reference-name grammar, **anchored** — printable ASCII, no whitespace,
  backslash, comma, quotes, brackets, braces, not starting with `*` or `=`.
  (A tab-separated defline in an unindexed FASTA yields an invalid name and
  throws here.)

### Two resolvers, strict and total

- `getCanonicalRefName(name)` — strict: throws if aliases not loaded,
  `undefined` if the name is unknown. Alias table first, then the lowercase
  index.
- `getCanonicalRefName2(name)` — **total** and the default for anything
  user-supplied or arriving from outside (features, RPC results, session
  specs, URLs): returns the input unchanged when aliases aren't loaded yet or
  the name is unknown. The doc rule: never hand-roll
  `getCanonicalRefName(x) ?? x` (the throw path).

Main-thread reading rules (root CLAUDE.md "Names" +
`agent-docs/reference/REFNAME_NAMESPACES.md`):

- Match user text over `allRefNames` (superset of canonical), resolve hits to
  canonical, emit by walking `regions` (order + dedupe).
- A display reading its **own** state uses `canonicalizeViewRefName`
  (`@jbrowse/core/util`); config slots/URLs are untrusted text.
- Assembly names off track configs must be canonical
  (`canonicalAssemblyNames`) _and_ present (`assemblyManager.has`);
  comparing two spellings is `isSameAssemblyName`, never `===`.

### The adapter→canonical map and the RPC boundary

`loadRefNameMap.ts`: for a _feature_ adapter, `loadRefNameMap(assembly,
adapterConf, opts)` calls `CoreGetRefNames` over RPC (worker has no assembly
manager) to get the **file's** refNames, then builds
`result[canonical(adapterName) ?? adapterName] = adapterName` — a total map
canonical→file. This is memoized per (assembly, adapter-config) in
`adapterLoads`. On the outbound side, `renameRegionsIfNeeded`
(`packages/core/src/util/renameRegions.ts`) rewrites a request's `regions[]`
into adapter spelling **destructively inside `serializeArguments`**, and sets
`originalRefName` to the FASTA name so BAM/CRAM can fetch reference sequence.

So `refName` has **two namespaces**: canonical on the main thread,
adapter/file spelling after the RPC boundary. The rename is one-way; the
return direction has no layer-level mechanism — six plugins each invented a
workaround (canonicalize on receipt via `getCanonicalRefName2`, synteny being
the worst case with two renamed channels and a dictionary re-intern step).
This is the single largest documented wart; REFNAME_NAMESPACES.md argues for a
per-method return rename "declared per method", starting from the alias table.

### Mismatch detection — `refNameMismatch.ts`

`detectRefNameMismatch`: only verdict reachable from names alone is the
**empty intersection** — both lists non-empty and no adapter name resolves
(canonicalized) into the assembly's set. Partial overlap is normal (a
sample-specific VCF). Verdict stored as
`{ assemblyName, adapter: {names[0..5], total}, assembly: {...} }` keyed by
adapter cache key; rendered via `refNameMismatchMessage` as a
TrackLabelRefNameWarning, never thrown — a wrong guess must not take a working
track away. Note it is deliberately blind when aliases _are_ configured and
working.

---

## 5. Sequence access

### Adapters that serve sequence bytes

All in `plugins/sequence` (registered via `Core-guessAdapterForLocation` so a
bare `uri` picks one):

| Adapter                 | Config slots (after shorthand)                                                                                          | File format                                                                                          |
| ----------------------- | ----------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `BgzipFastaAdapter`     | `fastaLocation`, `faiLocation` (`uri + '.fai'`), `gziLocation` (`uri + '.gzi'`), `metadataLocation` (optional YAML)     | bgzip-compressed FASTA + samtools faidx `.fai` + bgzip `.gzi`                                        |
| `IndexedFastaAdapter`   | `fastaLocation`, `faiLocation`, `metadataLocation`                                                                      | plain FASTA + `.fai`                                                                                 |
| `TwoBitAdapter`         | `twoBitLocation`, `chromSizesLocation` (needed — the 2bit has no random-access index JBrowse trusts for contig lengths) | UCSC `.2bit`                                                                                         |
| `ChromSizesAdapter`     | `chromSizesLocation`                                                                                                    | `name<TAB>length` text; **lengths only, no residues** — implements `RegionsAdapter` but not sequence |
| `UnindexedFastaAdapter` | whole-file in memory                                                                                                    | unindexed FASTA (main-thread scale only)                                                             |

`deriveFastaLocations` (`plugins/sequence/src/chromSizesUtils.ts`) is the
shared shorthand: `{ uri } → { fastaLocation: {uri}, faiLocation: {uri}.fai }`;
Bgzip adds `.gzi`. `FastaAdapterBase` is shared by indexed+bgzip.
`BaseSequenceAdapter` (`packages/core/src/data_adapters/BaseAdapter/BaseSequenceAdapter.ts`)
defines the interface: `getRegions()` + `getSequence(region)` (default
implementation routes through `getFeaturesArray` and reads the `seq`
attribute).

### How sequence is fetched at runtime

- `getSequenceSubAdapter` (`packages/core/src/data_adapters/getSequenceSubAdapter.ts`):
  any feature adapter that needs reference bases (CRAM, motif search…) gets the
  sequence adapter from **the assembly automatically** — `CoreGetRefNames`/
  `getFeatureAdapter` prime every adapter's `sequenceAdapterConfig` from the
  assembly the track is displayed against. A hand-written `sequenceAdapter`
  slot on a track is an anti-pattern (validate warns). If the assembly's
  adapter carries no residues (ChromSizes), resolution fails with a named
  error.
- `fetchSeq` (`packages/core/src/util/fetchSeq.ts`): the main-thread helper —
  canonicalizes the refName (`getCanonicalRefName2`), translates it to the
  FASTA spelling (`getSeqAdapterRefName`), then RPC `CoreGetSequence` with the
  snapshot of the sequence adapter config.
- `getSequenceAdapterConfig(assembly)` (`assemblyManager/getSequenceAdapterConfig.ts`):
  the sequence adapter config as a **plain snapshot** (MST nodes can't cross
  trees/RPC). This same snapshot is forwarded inside `CoreGetRefNames` calls
  so BAM/CRAM adapters can cache it.
- Caching: adapter _instances_ are cached per (sessionId, config) in
  `dataAdapterCache.ts` (`getAdapter`/`adapterConfigCacheKey`); per-adapter
  refName maps are cached on the assembly (`adapterLoads`); adapter instance
  setup (index downloads) memoized via `cachedSetup` in `BaseAdapter`.
  Sequence _residue_ caching is per-track (e.g. the reference-sequence
  display's `rpcDataMap`), not in the assembly layer.

### The sequence track

`ReferenceSequenceTrack` (`plugins/sequence/src/ReferenceSequenceTrack/`) is an
ordinary track type whose config is essentially just `adapter` (plus standard
base-track fields); its display (`LinearReferenceSequenceDisplay`) renders
translation rows driven by `assembly.getGeneticCodeId(refName)` and can export
FASTA (`saveTrackFormats/fasta.ts`). The assembly's `sequence` slot _is_ this
track's config; there is no separate "assembly sequence" object.

---

## 6. Genetic codes

`packages/core/src/assemblyManager/geneticCodes.ts`:

- `geneticCodes` config slot: `{ refName: NCBI transl_table id }`, e.g.
  `{ chrM: 2 }` (vertebrate mitochondrial), `{ chrPltd: 11 }`. Frozen (plain
  JSON in config).
- `geneticCodesLocation`: optional sidecar TSV `refName<TAB>geneticCodeId`
  (`#` comments), parsed by `getGeneticCodesFromFile`; parsed ids go through
  `parseTranslTable` (`packages/core/src/util/geneticCodes.ts`), which accepts
  numeric ids and NCBI table names.
- Resolution `lookupGeneticCodeId(refName, refNameAliases, [inlineMap,
  loadedMap])`: **inline config wins over the sidecar file**; both the query
  name and map keys are canonicalized through the alias table (a map keyed by
  RefSeq accessions resolves against a UCSC-canonical assembly); first map
  with an entry wins; default is the **standard code (id 1)**.
- CDS-level translation ignores all of this and reads the GFF `transl_table`
  attribute directly.
- Consumer: the reference sequence track's translation rows.

---

## Mab notes

- **The assembly is small.** The transferable core is: a `name` + aliases key,
  a contig list `(refName, length)` in file order, a canonicalization table
  `alias → canonical` plus a lowercase index, a `canonical ↔ file` spelling
  map, and optional sidecars (cytobands, genetic codes, display name). In Rust
  that's a couple of structs plus two `HashMap<String, String>`; the whole
  assembly could live in one `Arc<AssemblyData>` built once.
- **Make canonicalization total from day one.** JBrowse's biggest documented
  wart is two refName namespaces across its worker boundary, patched per
  plugin (six ad-hoc fixes; REFNAME_NAMESPACES.md §"What is done, and what is
  still open" calls for a declared per-method rename). In Rust, make it a type:
  `CanonicalRefName` vs `AdapterRefName` (newtype over String) so a name
  crossing the fetch boundary must be converted. JBrowse sketched exactly this
  (branded types, REFNAME_NAMESPACES.md §"Branding") but can't retrofit it.
  Normalize on **receipt** through the alias table, not by inverting the
  outbound map (which keeps only one file spelling per contig and is not
  total).
- **Accept the SAM refName grammar verbatim and validate at load** — it's 15
  lines, it rejects tab-separated deflines, and every downstream locstring/URL
  depends on it.
- **0-based half-open `[start,end)` interbase everywhere**; adopt
  `region.reversed` (a flag, not recomputed coordinates) for reverse-complement
  display in comparative views.
- **Derive, don't persist.** JBrowse persists only the config; regions, alias
  maps, and per-adapter maps are all lazily derived and memoized. A Mab
  assembly config of `{ name, uri }`-plus-inference (extension → adapter,
  derive sibling index paths) is a good default; keep the explicit config as
  the escape hatch.
- **Chrom.sizes is a valid minimal backend**: an assembly needs only
  `(refName, length)` pairs; sequence is optional. Design so a
  lengths-only source satisfies the "which contigs exist" contract and
  sequence queries fail with a clear "adapter provides no sequence" error.
- **Empty-intersection is the only diagnosable name mismatch**; partial overlap
  is normal. Record it as a diagnostic (keyed by the data source) rather than
  an error — JBrowse's discipline of never taking a working track away on a
  guess is worth copying.
- **Cache per (assembly, data-source) maps, evict on failure, memoize the
  promise** — the `adapterLoads` QuickLRU pattern is directly portable.
- Cytoband file: copy UCSC `cytoBand.txt` format but note JBrowse inherits an
  end off-by-one from the 1-based file; Mab should subtract one when ingesting.
