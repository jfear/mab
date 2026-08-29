# 01 — The Reference Genome Model (IGV v3.0.0-beta.4)

TL;DR: IGV's genome is a thin object: an id/display name, an ordered list of tiny `Chromosome` records
(name, index, length), a pluggable `Sequence` backend (indexed FASTA, block-compressed FASTA, 2bit,
in-memory), a chromosome-alias source (chr1 vs 1), a cytoband source, and whole-genome-view offsets.
Nearly everything heavy (sequence bytes, cytobands, aliases) is lazy, file-backed, and cached behind
small wrapper objects.

## Core types

### `org.igv.feature.Chromosome`

Deliberately minimal. Fields:

- `String name` — canonical name as used in the genome's own sequence (e.g. `chr1` for UCSC sets).
- `int index` — position in the chromosome ordering (transient, assigned at load).
- `int length` — chromosome length in bp.
- No cytobands, no sequence, no aliases live here. `equals`/`hashCode` on `(name, length)`.

### `org.igv.feature.genome.Genome`

Central model (`Genome(String id, List<Chromosome>)` minimal ctor; `Genome(GenomeConfig)` full ctor). Fields:

- `id`, `displayName` — id may be a path/URL; `displayName` derived from config (`name` → `scientificName` → `organism` → `description` → `id`), often `"Name (id)"`.
- `Map<String, Chromosome> chromosomeMap` — name → chromosome (insertion order from index/chrom.sizes).
- `List<String> chromosomeNames` — ordered names (file order; orderable via config `chromosomeOrder`).
- `List<String> longChromosomeNames` — chromosomes included in the whole-genome view.
- `Map<String, Long> cumulativeOffsets` — lazily computed start offset of each long chromosome for the whole-genome view; coordinates there are kb (see below).
- `Sequence sequence` — always wrapped in `SequenceWrapper` (tile cache).
- `ChromAliasSource chromAliasSource` + `Map<String,String> chrAliasCache` — alias resolution.
- `CytobandSource cytobandSource` — cytobands or pseudo-cytoband fallback.
- `String nameSet` — which alias column is the display name ("ucsc" default, "ncbi", …).
- Misc: `species` (from bundled `speciesMapping.txt` resource, prefix-matched), `ucscID`, `blatDB`, `defaultPos`, `homeChromosome` (`"all"` when WGV enabled), `liftoverMap` (set externally), `trackHubs`/`genomeHub` (first hub is "the genome hub"), `FeatureDB featureDB` (name→feature lookup), MANE/rsDB bigBed feature sources, `annotationResources` (default tracks).

Coordinate conventions:

- `getSequence(chr, start, end)` — **0-based start, end-exclusive** ("UCSC style"); end clamped to chromosome length; returns `null` when unknown chromosome or empty interval.
- `getReference(chr, pos)` — single base, 0-based; returns `0` if unavailable.
- Whole-genome coordinates: `getGenomeCoordinate(chr, bp)` = `(cumulativeOffset + bp) / 1000` — **kbp units, integer**; `getChromosomeCoordinate(genomeKBP)` converts back. Whole-genome view is a kilo-basepair linear layout, so per-base precision is intentionally lost at WGV zoom.
- Cytoband features are 0-based half-open like everything else (`Cytoband.start`/`end`).

Whole-genome view gating: enabled iff `config.wholeGenomeView` && chromosome list available && >1 chromosome && ≤ `MAX_WHOLE_GENOME_LONG` (100) long chromosomes.

`computeLongChromosomeNames()` heuristic:

- <100 chromosomes: keep chromosomes with length > 10% of the mean length.
- ≥100 chromosomes: sort by length descending, cut at the first gap where a chromosome is <30% of the previous one (`delta/len > 0.7` breaks).

### `org.igv.feature.genome.Sequence` (interface)

- `byte[] getSequence(String seq, int start, int end)` — 0-based half-open; `null` if unknown sequence.
- `byte getBase(String seq, int position)`
- `List<String> getChromosomeNames()`, `int getChromosomeLength(String)` (−1 if unknown), `List<Chromosome> getChromosomes()`, `boolean hasChromosomes()`.

Implementations:

- `org.igv.feature.genome.fasta.FastaIndexedSequence` — random access via `.fai`.
- `org.igv.feature.genome.fasta.FastaBlockCompressedSequence` — subclass for bgzf FASTA (`.gz` + `.gzi` + `.fai`).
- `org.igv.ucsc.twobit.TwoBitSequence` — UCSC 2bit.
- `org.igv.feature.genome.InMemorySequence` — `Map<String, byte[]>`; used for Genbank-loaded genomes.
- `org.igv.feature.genome.SequenceWrapper` — caching decorator always applied by `Genome`.

### `org.igv.feature.NamedFeature` (htsjdk) / `org.igv.feature.IGVNamedFeature`

IGV reuses htsjdk's `NamedFeature` interface (name, 0-based half-open `getStart()`/`getEnd()`, `getChr()`) via `IGVNamedFeature`, which adds `getDisplayName(property)`. `org.igv.feature.BasicFeature` (extends `AbstractFeature`) is the concrete workhorse; feature attributes live in `Map<String,String> attributes` (LinkedHashMap, insertion-ordered) on `AbstractFeature`.

## `GenomeManager` lifecycle (`org.igv.feature.genome.GenomeManager`)

Singleton; exactly **one genome loaded at a time** (`currentGenome`).

- `loadGenomeById(id)`: if `id` exists as file/URL → use directly; else look up `GenomeListItem` via `GenomeListManager` (user + downloaded lists) then `HostedGenomes` (remote hosted lists); then `loadGenome(path)`.
- `loadGenome(path)`: picks a loader via `GenomeLoader.getLoader(path)` by extension:
  - `.genome` → `DotGenomeLoader` (legacy zip archive)
  - `.gbk`/`.gb` → `GenbankLoader`
  - `.chrom.sizes` → `ChromsizesLoader` (minimal genome, no sequence)
  - `.json` → `JsonGenomeLoader`
  - `*hub.txt` → `HubGenomeLoader`
  - anything else → `FastaGenomeLoader` (fasta/fasta.gz/2bit). Gzipped fasta requires both `.gzi` and `.fai` or it refuses.
- After loading: applies user-defined alias overrides from genome cache dir (`<id>_alias.tab`), adds a `GenomeListItem` to the pulldown, calls `setCurrentGenome` which fires `GenomeChangeEvent` on the `IGVEventBus`, restores genome default annotation tracks (`getAnnotationResources()` → `ResourceLocator`s; `.genome`/`.gbk` also get a `geneTrack`).
- `GenomeLoader.loadSequenceMap()` reads `sequenceMap.txt` in the genome cache dir (id → local file) at startup.
- Hosted genomes: `HostedGenomes.readRecords()` fetches the IGV genome list from `GENOMES_SERVER_URL` (with a backup URL) plus UCSC Genark (`https://hgdownload.soe.ucsc.edu/hubs/UCSC_GI.assemblyHubList.txt`), keyed by `assembly`. `GenomeUtils.isDeprecated` flags FASTA URLs hosted on old amazonaws buckets and triggers `updateGenome` → re-download via `GenomeDownloadUtils.downloadGenome(config, …)`.
- Genome deletion (`deleteDownloadedGenomes`) removes cached archives, per-id data dirs, and legacy local fasta + `.fai` from the genome cache directory.

### Genome definition formats

**`GenomeConfig` (`org.igv.feature.genome.load.GenomeConfig`)** — JSON-ish config shared by all modern loaders (GSON from genome JSON, or built from hub). Key fields (names match the JSON keys, `genome.json` schema):
`id, name, fastaURL, indexURL, gziIndexURL, twoBitURL, twoBitBptURL, nameSet, defaultPos, description, blat, chromAliasBbURL, chromSizesURL, infoURL, cytobandURL, cytobandBbURL, ordered, blatDB, ucscID, aliasURL, accession, taxId, organism, scientificName, maneBbURL, maneTrixURL, rsdbURL, chromosomeOrder[] (string or comma-list), wholeGenomeView, hubs[], tracks[]`. Legacy `.genome`/`.gbk` loaders additionally inject `sequence` (in-memory), `cytobands` (LinkedHashMap chr→List<Cytoband>), `chromAliases` (List<List<String>> synonym rows).

`Genome` constructor precedence: `config.sequence` (in-memory) > 2bit > fasta. Chromosomes come from, in order: `chromSizesURL` (parsed by `ChromSizesParser`: whitespace-separated `name length` lines, order = file order, index assigned sequentially) → `sequence.getChromosomes()` (fasta index) → fasta index parsed explicitly (2bit without chrom.sizes case). If none, chromosome pulldown/whole-genome view are effectively disabled.

**Legacy `.genome` archive** (`GenomeDescriptor`, `DotGenomeLoader`): a zip with `property.txt` (Java Properties) containing keys `id, name, version, ordered, cytobandFile, geneFile, chrAliasFile, geneTrackName, url, sequenceLocation, compressedSequencePath`. Embedded cytoband/alias/annotation files are parsed into the config; the gene file becomes the `geneTrack` (`FeatureTrack` named by `geneTrackName`, default "Gene").

**FASTA-only genome** (`FastaGenomeLoader`): id = path, name = filename; creates a `.fai` locally if missing (never for remote); 2bit path sets `twoBitURL`.

**chrom.sizes genome** (`ChromsizesLoader`): minimal `Genome(id, chromosomes)` — no sequence, aliases via `ChromAliasDefaults`, home chromosome = first (or `all` if >1 long chr).

## Sequence storage & querying

### FASTA `.fai` (`FastaIndex`, `FastaIndexedSequence`)

`.fai` is 5 tab-separated columns: `name, size (bases), location (byte offset of sequence start), basesPerLine, bytesPerLine`. IGV's parser splits the name at whitespace (`SEQUENCE_NAME_SPLITTER`), keeps insertion order via `LinkedHashMap`. Query math (see doc comment): line-walk `startByte = position + startLine*bytesPerLine + offset`, read the whole byte span, then strip newline bytes while copying into the output buffer. Reads go through `IGVSeekableStreamFactory` with a 1 MB buffered stream opened per query (local or http/https/s3). `getBase()` is **not implemented** here — only the wrapper's tile cache provides single-base access.

### Block-compressed FASTA (`FastaBlockCompressedSequence`)

Extends the indexed reader; adds `.gzi` (little-endian: `long` count then pairs of `compressedOffset, uncompressedOffset`). Virtual position encoding: `(compressedOffset << 16) | offsetInBlock` (BGZF). `readBytes` binary-searches the gzi mappings, seeks the `BlockCompressedInputStream`, and reads the span.

### 2bit (`org.igv.ucsc.twobit`)

- Header: magic `0x1A412743` (detects endianness by retrying big-endian), version, sequenceCount, reserved.
- Index: either the embedded (legacy) index (`TwoBitIndex`) or an external BPT (`BPTree.loadBPTree(indexPath, 0)`, from `config.twoBitBptURL`) — with a BPT, chromosome _names_ cannot be enumerated, only searched.
- Per-sequence record (`SequenceRecord`): `dnaSize`, `nBlockCount` + `(start,size)` pairs, `maskBlockCount` + pairs, reserved word, then 4-bit packed DNA at `packedPos`. Records are cached in a name→record map after first lookup.
- `readSequence`: byte-level decode via lookup tables `byteTo4Bases` (order T,C,A,G, MSB first) and `maskedByteTo4Bases` (lowercase for soft-masked regions); N-blocks fill 'N'. Coordinates 0-based half-open. `getBase()` unimplemented; `getChromosomes()` returns null (`hasChromosomes() == false`), so 2bit genomes need a `chromSizes` URL or fasta index for chromosome metadata.

### Caching: `SequenceWrapper`

All sequence access goes through it. Tile cache: `ObjectCache<String, SequenceTile>` of **50 tiles**, tile size **1,000,000 bp** (`tileSize`, static; settable). Tiles are keyed `chr:startTile`, and a run of consecutive missing tiles is fetched as one bulk `getSequence` then split into tiles. Missing data is cached as zero-filled bytes. This is the _only_ single-base access path (`getBase` reads the position's tile).

Cytoband sources (`org.igv.feature.genome`): `CytobandMap` (in-memory map parsed from cytoband.txt: `chr start end name gieStain`, 0-based half-open), `CytobandSourceBB` (bigBed). Size guards on remote files: cytobandBB ≤ 1 MB, cytobandURL ≤ 100 KB, chrom.sizes ≤ 10 MB. When no cytobands exist, `Genome.getCytobands()` fabricates one full-length `gneg` band so the ideogram/nav widget still renders.

### Chromosome aliases (`chr1` vs `1`)

- `ChromAlias` — per-chromosome record: `chr` (canonical) + `Map<String,String> aliases` keyed by name-set (`"ucsc"`, `"ncbi"`, `"refseq"`, …); auto-includes a `**DEFAULT**` entry (`chr1`↔`1`).
- `ChromAliasSource` (abstract) — `aliasCache` map alias→record; `getChromosomeAlias(chr, nameSet)`, `search(alias)`; `add()` for overrides.
  - `ChromAliasDefaults` — synthesized rules from the genome id: strip/`add chr` prefix; `chrM`↔`MT`; NCBI numbering of X/Y (23/24 human, 21/22 mouse & rheMac depending on id prefix); accession version stripping (`ncbi-noversion`); gi\| name handling. `nameSet` keys display names.
  - `ChromAliasFile` (via `ChromAliasParser`) — tab-file of synonym rows.
  - `ChromAliasBB` — bigBed alias file: each BED record's name is the canonical chr and its attributes are aliases (attribute key = name-set); preloaded for the genome's chromosomes.
- `Genome.getCanonicalChrName(str)` — resolves through the source and memoizes in `chrAliasCache` (all synonyms of a record map to the canonical name); falls back to lowercasing the query; returns input if unresolved. User-defined aliases (from `<genomeId>_alias.tab` in cache dir or `config.chromAliases`) are added last so they win.
- Order/sort: `ChromosomeNameComparator` is now a plain lexicographic `String.compareTo` (historic natural-sort logic deprecated); the effective display order is file/index order or explicit `chromosomeOrder`.

## Species / attributes schema

- Species: `Genome.getSpecies()` consults bundled resource `speciesMapping.txt` (tab: id-prefix → species), prefix-matched against `ucscID`.
- `GenomeConfig` carries metadata: `organism`, `scientificName`, `taxId`, `accession`, `description`, `infoURL` — this is the modern replacement for the old `.genome` properties.
- Cytoband stain schema (`Cytoband`): `name`, `start`, `end` (0-based half-open), `type` char — `'p'` = gpos stain (from `gieStain` char 1, e.g. `gpos50`), `'n'` = negative/gneg, `'c'` = acen (centromere) — plus `short stain` intensity parsed from `gposNN` (default 100 when absent).

## Liftover (`org.igv.util.liftover`, brief)

- `Chain` — one UCSC chain: header fields `tName,tSize,tStart,tEnd,qName,qSize,qStart,qEnd,id` (positions as in chain files: target/query start offsets, 0-based; strand applied by the file's sign convention — IGV takes absolute values from tokens 5/6 & 10/11). Alignment lines build an `IntervalTree<int[]>` mapping target intervals → `[qStart,qEnd]` pairs (inserted gaps advance both coordinates).
- `Liftover` — `Map<String, Chain>` keyed by target chromosome; `load(path)` parses chain text; `map(Range)` returns query ranges, sorted and coalesced when contiguous. **Known limitation (TODO in source): assumes exactly one chain per target chromosome; multiple chains would need a tree-of-chains.**
- `Genome.liftoverMap` (`Map<String, Liftover>`) is set externally (session/property-driven), not populated by the genome loaders.

## Mab notes

- Model the genome as a small struct: id, display name, ordered chromosome list (name/index/length), plus trait-object backends for sequence, aliases, and cytobands — IGV's separation of `Chromosome` (tiny metadata) from `Sequence` (byte access) is worth keeping.
- Normalize coordinates once: 0-based half-open everywhere internally (matches htsjdk/BED/fasta-index math); convert to 1-based inclusive only at the UI layer. The one exception is the whole-genome view, which IGV treats as a kbp-resolution linear chromosome concatenation — an explicit `GenomeCoordinate` (offset table + per-chromosome lengths) is a clean Rust analogue.
- Alias handling: cache map (alias → canonical) over a per-chromosome record with named "name sets" (`ucsc`, `ncbi`, …) is simple and effective; user overrides must be applied last. A Rust version can be a `HashMap<String, Arc<AliasRecord>>` behind `RwLock`.
- Sequence backends should be a trait with `sequence(chr, 0-based range) -> Option<Vec<u8>>`; implement `.fai` FASTA (easy, spec is 5 columns), bgzf+gzi FASTA (virtual offset = `compressed << 16 | in-block`), and 2bit (4-bit packed, T/C/A/G nibble order, N and soft-mask block tables). Tile cache (1 Mb tiles, LRU ~50) sits above the trait, not inside it — single-base queries only exist through the cache.
- Chromosome metadata source of truth: prefer chrom.sizes/fasta-index over deriving from the sequence backend; note 2bit + BPT index cannot enumerate contigs.
- Whole-genome inclusion heuristics (10% of mean, or gap-cut at ≥70% drop for contig-heavy assemblies) are cheap, self-tuning, and directly portable.
- Genome config as a plain serializable struct (IGV's `GenomeConfig` JSON) with stable field names is a good pattern for session files and a Mab genome registry; drop the legacy `.genome` zip format entirely.
- Liftover: chain parsing → interval tree per chain is straightforward in Rust (e.g. `rust-lapper`-style); fix the one-chain-per-target assumption by using an interval tree of chains keyed by target name.
- Ambiguities to check before implementing: `Sequence.getBase()` is unimplemented in all three file-backed backends (only the wrapper provides it) — decide whether Mab needs it; `Genome.getSequence` silently returns `null` (not an error) for unknown chromosomes — Rust should use `Option`/`Result` deliberately; `FastaIndex` truncates contig names at first whitespace, which differs subtly from htsjdk/samtools behavior for headers with spaces.
