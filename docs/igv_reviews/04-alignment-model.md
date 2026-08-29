# 04 — Alignment / Read Model (IGV v3.0.0-beta.4)

**TL;DR** — IGV's read model is a single interface `org.igv.alignment.Alignment`
(a `LocusScore` with flags, CIGAR-derived blocks/gaps, mate info, and tag
attributes) with one full-fidelity implementation (`SAMAlignment`, wrapping an
htsjdk `SAMRecord`) and one lossy "reduced memory" variant. Everything else —
coverage, packing, downsampling, base modifications — is derived per loaded
interval and cached only for as long as the interval is in view. Coordinates are
**0-based half-open** everywhere inside IGV; conversion from htsjdk's 1-based
inclusive happens at a single point in the adapter.

## 1. Core interfaces

### `org.igv.alignment.Alignment` (extends `org.igv.feature.LocusScore`)

| Member                                                                         | Notes                                                                        |
| ------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| `getReadName()`                                                                | non-null; default `""`                                                       |
| `getReadSequence()`                                                            | read bases (not reference)                                                   |
| `getAlignmentStart() / getAlignmentEnd()`                                      | 0-based; `[start, end)` end-exclusive                                        |
| `contains(double location)`                                                    | membership test against `[start,end)`                                        |
| `getAlignmentBlocks()` → `AlignmentBlock[]`                                    | contiguous matched segments (incl. soft-clip blocks when shown)              |
| `getInsertions()` → `AlignmentBlock[]`                                         | insertion blocks carry read bases but zero reference span                    |
| `getCigarString() / getCigar()`                                                | `"*"` default when absent                                                    |
| `getGaps()` → `List<Gap>`                                                      | deletions (`D`), splice skips (`N`); `SpliceGap` subclass for `N`            |
| `getInferredInsertSize()`                                                      | TLEN, unchanged from record                                                  |
| `getMappingQuality()`                                                          | MAPQ                                                                         |
| `getMate()` → `ReadMate`                                                       | mate chr (canonicalized), 0-based mate start, strand, mapped flag            |
| `setMateSequence(String)`                                                      | filled in during tile load for unmapped mates                                |
| `getReadStrand() / isNegativeStrand()`                                         | strand                                                                       |
| `isPaired / isProperPair / isFirstOfPair / isSecondOfPair`                     | flags                                                                        |
| `isMapped / isPrimary / isSupplementary / isDuplicate / isVendorFailedRead`    | flags                                                                        |
| `getBase(double position) / getPhred(double position)`                         | reference-position lookup; delegates to containing block                     |
| `getAttribute(String key)`                                                     | SAM tag lookup; 2-char keys only, plus virtual `TEMPLATE_ORIENTATION`        |
| `getYcColor()`                                                                 | explicit `YC` tag color                                                      |
| `getSample() / getReadGroup() / getLibrary()`                                  | derived from read group `SM/DS/LB`                                           |
| `getPairOrientation() / getFirstOfPairStrand() / getSecondOfPairStrand()`      | derived in constructor                                                       |
| `getBaseModificationSets()` → `List<BaseModificationSet>`, `getSmrtKinetics()` | optional parsed tags                                                         |
| `getClippingCounts()`                                                          | `ClippingCounts.fromCigar(cigar)` (hard/soft clip per side)                  |
| `finish()`                                                                     | per-record post-load hook (base-mod parsing, SBX trimming)                   |
| `getSpecificAlignment(double location)`                                        | most specific sub-alignment (used by `PairedAlignment`/`SupplementaryGroup`) |
| `getClusterName/setClusterName/getHapDistance`                                 | haplotype/grouping annotations                                               |

### `org.igv.alignment.AlignmentBlock`

A contiguous piece of the read projected onto the reference:

| Field                         | Meaning                                                                                                |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ |
| `start`                       | 0-based reference start of the block                                                                   |
| `offset` (`getBasesOffset()`) | offset into the parent read's base/quality arrays                                                      |
| `bases`, `qualities`          | `org.igv.alignment.ByteSubarray` — a window (offset,len) over the shared record arrays, **not copies** |
| `basesLength`                 | number of bases in the block                                                                           |
| `padding`                     | `P` operator padding added to `getLength()` (insertion rendering)                                      |
| `cigarOperator`               | `M`/`=`/`X`/`S`/`I`                                                                                    |
| `softClipped`                 | true for `S` blocks                                                                                    |
| `pixelStart/pixelEnd`         | cached pixel range for hit-testing (`containsPixel`)                                                   |
| `indexOnRead`                 | == `getBasesOffset()`                                                                                  |

`AlignmentBlockImpl` is the concrete class used by `SAMAlignment`;
`ReducedMemoryAlignmentBlock` is a bases-free (start+len+softClip only) version.
Qualities are optional (missing → default byte `126`, `getActualQuality` returns
`null`).

### Supporting types

- `Gap` — `{int start (0-based), int nBases, char type}`; `SpliceGap` adds
  `flankingLeft/flankingRight` (sizes of flanking blocks, used to draw splice
  arcs). `getDeletionAt(pos)` filters `type == 'D'`.
- `ReadMate` — `{String chr, int start (0-based), boolean negativeStrand, boolean unmapped}`.
- `ByteSubarray` — zero-copy slice over the `SAMRecord` base/qual byte arrays.
- `PairedAlignment` — wraps both ends of a pair into one `Alignment` for
  "view as pairs"; `SupplementaryAlignment`/`SupplementaryGroup` do the same for
  chimeric records (`SA` tag), cached per-record in `SAMAlignment` under
  `CacheKey.SA_GROUP`.
- `LinkedAlignment` — groups reads sharing a link tag when
  `RenderOptions.isLinkedReads()` (e.g. 10x `BX`).
- `ClippingCounts` — pure data holder of hard/soft clip counts per side.

## 2. Implementations & the htsjdk boundary

- **`org.igv.alignment.SAMAlignment`** (`reader/` produces it from
  `SAMRecord`). The adapter:
  - Converts coordinates once in the constructor: `end = record.getAlignmentEnd()`
    (htsjdk 1-based inclusive already equals IGV 0-based exclusive end);
    `start = record.getAlignmentStart() - 1`. Comment in source: _"SAMRecord is
    1 based inclusive. IGV is 0 based exclusive."_
  - Canonicalizes contig names through the current `Genome` (`getCanonicalChrName`).
  - Flags are copied into a local `int flags` and all boolean accessors are
    manual bitmask tests (SAM flag constants re-declared: `0x1..0x800`).
  - `createAlignmentBlocks()` walks the CIGAR (see §3).
  - Tags stay in the `SAMRecord`; `getAttribute` delegates. Mate info captured
    eagerly as a `ReadMate`.
  - Holds a reference to the live `SAMRecord` (not a copy).
- **`ReducedMemoryAlignment`** — built from a full `Alignment` when the
  `SAM_REDUCED_MEMORY_MODE` preference is set; keeps only name/chr/start/end/
  strand/cigar-string, merges adjacent blocks if the gap < `indelLimit`
  (`SAM_SMALL_INDEL_BP_THRESHOLD`), and drops insertions/deletions smaller than
  `indelLimit`. Discards bases, qualities, tags.
- **`DotAlignedAlignment`** / `sbx.NullAlignment` — minimal implementations for
  the legacy "aligned" format and as a null object; SBX (Singular Genomics)
  reads use `SbxUtils` heuristics (low baseQ tail trimming, quality-score
  fingerprinting to detect the platform).
- **Readers** (`org.igv.alignment.reader`): `AlignmentReader<T>` interface —
  `query(sequence, start, end, contained)` plus `getSequenceNames()`,
  `getSequenceDictionary()`, `hasIndex()`, `iterator()`. `AlignmentReaderFactory`
  dispatches by extension: `sam` → `SAMReader` (line parser), `bam`/`cram`/htsget
  → `BAMReader` (htsjdk `SamReader` via a pooled `SamReaderPool`),
  `*.list` → `MergedAlignmentReader` (logical merge, per-reader contig-name
  maps), plus legacy `GeraldReader`, `PSLAlignmentParser`, `DotAlignedParser`.
- **CRAM**: `org.igv.alignment.cram.IGVReferenceSource` implements htsjdk
  `CRAMReferenceSource`; it fetches bases from the loaded IGV `Genome`
  (canonicalized contig names, uppercased, region queried 0-based) and is
  registered on the `SamReaderFactory` used by `SamReaderPool`.
  `cram.ReferenceDiskCache` caches reference chunks on disk. CRAM failure is
  surfaced as "possible sequence mismatch (wrong reference for this file)".
- **Filtering/query** happens inside `BAMReader` via htsjdk
  `SamReader.query(contig, start, end, contained)` — IGV does not maintain its
  own index for BAM; `.bai`/`.crai`/`.csi` are used through htsjdk. Index
  creation is provided only for plain-text formats (`AlignmentIndexer`,
  `SamIndexer`, `DotAlignedIndexer`).

## 3. CIGAR → blocks / insertions / gaps (`SAMAlignment.createAlignmentBlocks`)

Single pass over a pre-parsed operator list (own lightweight
`SAMAlignment.CigarOperator` struct `{char operator, int nBases}`):

- `H` — skipped entirely (length via `getLeadingHardClipLength`).
- Match ops: `M`/`=`/`X` and, if the "show soft clips" preference
  (`SAM_SHOW_SOFT_CLIPPED`) is on, `S` too → `AlignmentBlockImpl`.
- `D` → `Gap(start, n, 'D')`; `N` → `SpliceGap(start, n, 'N', flankL, flankR)`;
  both also recorded in a parallel `char[] gapTypes`.
- `I` → insertion `AlignmentBlockImpl` (bases, zero reference span, with `P`
  padding carried to the next block); gapTypes records a `ZERO_GAP` (`'O'`)
  where two match blocks are split by an insertion, so gapTypes stays aligned
  with the block/gap sequence.
- If showing soft clips, `start` is shifted left by the leading `S` count before
  building blocks; trailing soft-clip block is marked separately.
- `"*"` CIGAR (unmapped) → one block spanning the read bases.

## 4. Loading pipeline

`org.igv.alignment.AlignmentDataManager` (one per file, shared by alignment +
coverage + junction tracks):

- Owns an `AlignmentReader`, an `AlignmentTileLoader`, a synchronized
  `intervalCache` of `AlignmentInterval`s, `PEStats` per library, and the union
  set of `BaseModificationKey`s seen.
- `load(frame, renderOptions, displayMode, expandEnds)`:
  - No-op for whole-genome view (`Globals.CHR_ALL`) or when
    `frame extent > getVisibilityWindow()` — visibility window is preference
    `SAM.MAX_VISIBLE_RANGE` (in kb, ×1000).
  - Expands the query window to ±2 screens (min of `4×(end-start)` and
    max-visible-range), then `AlignmentTileLoader.loadTile`.
- `AlignmentTileLoader.AlignmentTile.loadTile` iterates the reader query once,
  building everything derived per interval:
  - **Mate sequences** for unmapped mates are filled by cross-caching read
    names in two `ObjectCache`s (mapped/unmapped mates, cap 1000 each).
  - **Inline filtering** (there is no filter class hierarchy in this version):
    unmapped, duplicates (option `FILTER`), vendor-failed, non-primary,
    supplementary, `MAPQ < SAM.QUALITY_THRESHOLD`, read groups listed by
    `ReadGroupFilter` (an external text file of read-group names), and
    `AS` tag < `SAM.ALIGNMENT_SCORE_THRESHOLD`.
  - Detects `YC` (color tags), `BX` (10x), `HP` (phasing) tags on the fly.
  - Accumulates `PEStats` (insert-size percentiles `SAM.MIN/MAX_INSERT_SIZE_PERCENTILE`,
    expected pair orientation) per library for pair coloring.
  - **Downsampling** (see §7), **coverage counts** (see §6), splice junctions
    (`SpliceJunctionHelper` — counts junctions by `(start,end)` pair with
    per-strand counts), `InsertionManager` registers large insertions for
    click-to-expand.
  - Guards against memory pressure: every 1000 records checks
    `memoryTooLow()`; on failure it GCs, cancels and returns a partial tile.
- `AlignmentInterval` (extends `Locus`) is the cached unit: `{chr, start, end,
  List<Alignment>, AlignmentCounts, SpliceJunctionHelper,
  List<DownsampledInterval>, PackedAlignments, ReferenceFrame}`. Cache trimming
  keeps only intervals overlapping some `ReferenceFrame` (O(#frames×#intervals)).
- Whole-genome / low zoom: neither track renders; a **precomputed coverage
  file** (`.coverage`, via `org.igv.data.CoverageDataSource`, "Load coverage
  data" menu) can substitute for the derived coverage track at high zoom-out.

## 5. Row packing (`org.igv.alignment.AlignmentPacker`)

`packAlignments(interval, renderOptions, frame, displayMode)` →
`PackedAlignments` (a `LinkedHashMap<String, List<Row>>`; `Row` is just a list
of `Alignment`s; keys are group labels):

- **Grouping**: `AlignmentTrack.GroupOption` (STRAND, SAMPLE, READ_GROUP,
  LIBRARY, FIRST_OF_PAIR_STRAND, TAG, PAIR_ORIENTATION, MATE_CHROMOSOME,
  CHIMERIC, SUPPLEMENTARY, BASE_AT_POS, INSERTION_AT_POS, MOVIE, ZMW, CLUSTER,
  READ_ORDER, LINKED, PHASE, REFERENCE_CONCORDANCE, MAPPING_QUALITY, SELECTED,
  DUPLICATE, NONE). Some options default to reverse-sorted (`reverse` flag).
  Each group is packed independently; group order comes from a comparator
  (including a `PairOrientationComparator` following a canonical orientation
  order).
- **Dense mode** (`Track.DisplayMode.SPARSE`-style, the default):
  1. Optional pairing by read name into `PairedAlignment` (secondary alignments
     excluded) via a `Map<String, PairedAlignment>`.
  2. Bucket alignments by `start − rangeStart` into a `BucketCollection`:
     `DenseBucketCollection` (array of buckets) when span < 10 Mb,
     `SparseBucketCollection` (sorted map) otherwise (`AlignmentPacker.tenMB`).
     Each bucket is a `PriorityQueue` ordered by **read length descending**.
  3. Sweep: pop from the bucket at the current position, place into the current
     `Row`, jump to `alignment.end + MIN_ALIGNMENT_SPACING (2)`, repeat; when no
     bucket remains before the end of the range, start a new row. Longest-first
     bucketing makes each row greedily accept long reads early.
- **Full mode** (`Track.DisplayMode.FULL`): one row per alignment (filtered to
  the visible range), i.e. no packing.
- `linkByTag` may merge alignments sharing a tag value into `LinkedAlignment`s
  before packing.

## 6. Coverage (`AlignmentCounts`, `CoverageTrack`)

- **`org.igv.alignment.AlignmentCounts`** interface (per loaded interval):
  `getTotalCount / getTotalPositiveCount / getTotalNegativeCount /
  getTotalQuality(pos)`, `getPosCount/getNegCount(pos, base)`,
  `getQuality(pos, base)`, `getDelCount/getInsCount(pos)`, `getMaxCount(origin,
  end)`, `getBases()`, plus snp helpers `isConsensusMismatch /
  isConsensusDeletion / isConsensusInsertion(pos, snpThreshold)`.
- Implementations chosen in `AlignmentTile` by span:
  - `DenseAlignmentCounts` (≤10 Mb): parallel arrays per position —
    `Map<Byte,int[]> posCounts/negCounts/qualities`, plus `posTotal/negTotal/
    del/ins/totalQ` int arrays and a coarse `maxCounts` array (one entry per
    `MAX_COUNT_INTERVAL` positions) for fast auto-scaling.
  - `SparseAlignmentCounts` (>10 Mb): hash-based.
  - `ReducedMemoryAlignmentCounts` (reduced-memory mode): positions counted
    with a 25 bp resolution limit.
- Counting (`BaseAlignmentCounts.incCounts`) walks blocks/insertions/gaps per
  alignment, strand-aware; deletions increment `del[pos]`, insertions are keyed
  by (position, sequence) in `ins` structures (used for insertion markers).
- **Allele frequency / SNPs**: `snpThreshold` (preference `SAM.ALLELE_THRESHOLD`)
  is a _fraction_; a position is flagged as a mismatch when
  `Σ(non-ref base counts or their quality sums, if SAM.ALLELE_USE_QUALITY) ≥
  threshold × total`. Known SNPs can mask positions from a tab file
  (`chr \t 1-based location`). `isConsensusDeletion` requires del ≥ threshold×
  total over ≥ half the deletion width; `isConsensusInsertion` compares ins
  count against `threshold × (total + del)` (deletions counted as coverage).
- **`CoverageTrack`** renders per-base bars colored by strand, with mismatch
  coloring driven by `isConsensusMismatch`; hover shows a table of A/C/G/T
  counts with +/− strand split, percentages, DEL/INS counts, and HGVS/ClinVar
  links via the genome's reference base. Auto-scale is default (`globalAutoScale`).

## 7. Downsampling & `DownsampledInterval`

- `AlignmentDataManager.DownsampleOptions` `{boolean downsample, int
  sampleWindowSize, int maxReadCount}` from preferences
  (`SAM.DOWNSAMPLE_READS`, `SAM.SAMPLING_WINDOW`, `SAM.SAMPLING_COUNT`).
- Applied inside `AlignmentTile` during streaming, per **sampling window** (a
  bucket starting at each alignment's start, `alignmentStart ≥
  currentSamplingWindowStart + windowSize` closes it):
  - First `samplingDepth` distinct read names are kept.
  - Beyond that, **reservoir sampling**: replacement probability
    `depth / (depth + downsampledCount + 1)`; a random kept record within the
    window is replaced by the newcomer.
  - Reads sharing a read name (mates, supplementary) inherit the fate of the
    first-seen record — an `IndexableMap<String, List<Alignment>>` keeps both
    name lookup and random-access replacement.
- **`DownsampledInterval`** — `{int start, int end, int count}` (implements
  htsjdk `Feature`); `count` = reads removed in that window (incremented on
  reject _and_ whenever a reservoir replacement evicts a record). Rendered as a
  grey band whose shade scales with count (0→100), tooltip "N reads removed".
- Downsampled reads still contribute to coverage counts (`counts.incCounts`
  runs before downsampling), so coverage is unbiased; only the read rows are
  sampled.

## 8. Base modifications (`org.igv.alignment.mods`)

- Parsed lazily in `SAMAlignment.finish()`/`getBaseModificationSets()` from the
  `MM` (string) and `ML` (byte array) tags:
  `BaseModificationUtils.getBaseModificationSets(mm, ml, sequence,
  isNegativeStrand)` — supports multi-char ChEBI IDs, skip counts (`?`/`+`/`-`
  flags per the SAM spec), and validates that MM base counts match the read
  length (`validateMMTag`, warns ≤20 times).
- **`BaseModificationKey`** — `{char base (canonical as recorded), char strand
  ('+'/'-'), String modification}`; interned via a global cache; `getCanonicalBase()`
  complements the recorded base for `−` strand.
- **`BaseModificationSet`** — `{char base, char strand, String modification,
  Map<Integer, Byte> likelihoods}` where the map key is the **0-based offset in
  the read sequence as stored in the BAM** (left-to-right, not 5′→3′) and the
  value is the ML probability 0–255. Includes a name table for single-letter
  codes (`m`→5mC, `h`→5hmC, `f`→5fC, `c`→5caC, `a`→6mA, …).
- **`BaseModificationCounts`** — interval-level aggregation for the coverage
  track: `Map<BaseModificationKey, Map<Integer, ByteArrayList>>` for both
  `maxLikelihoods` (per position, per aligned read's ML value at that genomic
  position) and `nomodLikelihoods` (unmodified probability for reads covering
  the position but not modified there). Per-position counts support a
  likelihood threshold and optional inclusion of "no-mod" calls; the modified
  fraction can be drawn 1-color or 2-color (`BaseModificationRenderer`,
  `BaseModificationCoverageRenderer`) and filtered by
  `BaseModficationFilter`.

## 9. Track-level options (`org.igv.alignment.AlignmentTrack` + `RenderOptions`)

- **Visibility window**: `SAM.MAX_VISIBLE_RANGE` (kb); neither track loads or
  paints beyond it, and whole-genome (`CHR_ALL`) never renders reads.
- **Coloring** — `AlignmentTrack.ColorOption`: INSERT_SIZE, READ_STRAND,
  FIRST_OF_PAIR_STRAND, PAIR_ORIENTATION, READ_ORDER, SAMPLE, READ_GROUP,
  LIBRARY, MOVIE, ZMW, BISULFITE, NOMESEQ, TAG, NONE, UNEXPECTED_PAIR,
  MAPPED_SIZE, LINK_STRAND, YC_TAG, SPLIT, BASE_MODIFICATION,
  BASE_MODIFICATION_2COLOR, SMRT_SUBREAD_IPD/PW, SMRT_CCS_FWD/REV_IPD/PW,
  READ_NAME. Pair-orientation and insert-size coloring rely on `PEStats`
  percentiles/orientation computed during load. `ExperimentType` enum
  (OTHER, RNA, BISULFITE, THIRD_GEN, SBX, UNKOWN [sic]) is inferred from the
  file/platform and switches defaults (e.g. RNA → splice rendering, THIRD_GEN →
  SMRT kinetics panel).
- **Duplicates**: `DuplicatesOption` FILTER/SHOW; **grouping** as in §5;
  **sort** by start/strand/score etc.; **view as pairs**; **shade bases by
  quality**; **bisulfite context** (`BisulfiteContext` enum: C, CG, CHG, CHH,
  HCG, HCHG, HCHH) with `BisulfiteCounts`/`BisulfiteBaseInfo` per base.

## Mab notes

- IGV's central abstractions map cleanly to Rust: an `Alignment` trait with
  `AlignmentBlock`/`Gap` views over shared read buffers (IGV's `ByteSubarray`
  ≈ `&[u8]` slices) — prefer borrowed views over cloned arrays for memory.
- Pick one coordinate convention: 0-based half-open everywhere; convert
  exactly once at the parser boundary (IGV's single `getAlignmentStart() - 1`
  is a good model; document the soft-clip expansion of `start`).
- Precompute derived data (coverage arrays, junction counts, insertions,
  downsampling intervals) per loaded window in one streaming pass, keyed into a
  small interval cache evicted when out of view — IGV's `AlignmentInterval` is
  effectively a per-window memo. A Rust equivalent could be
  `Arc<LoadedInterval>` in an LRU.
- Coverage as struct-of-arrays (`Map<base,int[]>` per strand + total/quality
  arrays) is simple and fast; add a coarse running-max array for cheap
  autoscaling. Consider `u16`/`u32` counts depending on expected depth.
- Downsampling: windowed reservoir sampling that keeps mates/supplementary
  records of a sampled read together is subtle but valuable — the "same read
  name shares fate" rule is what makes pair rendering work after sampling.
- Row packing: bucket-by-start with longest-read-first priority queues, greedy
  row sweep with spacing = 2 bp. Straightforward to port; the 10 Mb
  dense/sparse switch is a memory heuristic, less relevant with good sparse
  structures.
- Filters in this IGV version are _not_ a class hierarchy — they're inline
  boolean checks during load (MAPQ, duplicate, secondary, supplementary, failed,
  read-group list, AS score). A trait-object `ReadFilter` chain in Rust would be
  a clean generalization; the `ReadGroupFilter` (external name list) shows the
  only user-configurable case.
- Base modifications: keep the `(canonical base, strand, code) → read-offset →
  ML byte` structure; the read-offset (not genomic) keying plus strand-complement
  canonical base rules are the tricky, spec-mandated parts.
- htsjdk boundary ≈ your BAM/CRAM decode layer: htsjdk provides parsing, index
  querying, and CRAM decode (with an injected reference source — the analogue is
  feeding your genome store to e.g. rust-htslib/needletail equivalents);
  everything above (`SAMAlignment` adaptation, canonical names, blocks/gaps) is
  application logic.
- Ambiguities to check before porting: `AlignmentQueryBuilder` does not exist in
  this source tree (querying is `AlignmentReader.query` + htsjdk); a `ReadFilter`
  hierarchy likewise does not exist (task assumption, not source).
