# 05 — Storage, Indexing, and Data Loading (IGV 3.0.0-beta.4)

TL;DR: IGV has three storage/query stacks: (1) tribble (htsjdk) text formats + linear/interval indexes or tabix/bgzf, (2) UCSC binary formats (BigWig/BigBed with B+ chromosome tree, R+ spatial tree, zoom levels), and (3) its own TDF precomputed binary format produced by igvtools. All remote access funnels through a seekable-stream abstraction backed by HTTP Range requests; caching is small fixed-size LRU maps, not a global memory budget.

## 1. `org.igv.util.ResourceLocator`

The universal handle for "where the data is" — file path, URL, or `htsget://` URI. Nearly all fields are now delegated to an embedded `TrackConfig` (`org.igv.feature.genome.load.TrackConfig`).

| Field                                                                                      | Getter/Setter                       | Notes                                                                                                                                                                                              |
| ------------------------------------------------------------------------------------------ | ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `path` → `TrackConfig.url`                                                                 | `getPath()` / `setPath()`           | Strips a `file://` prefix on set; `s3://` paths auto-detect an index (`detectIndexPath`: `.bam → .bai`, `.gz → .tbi` appended).                                                                    |
| index path → `TrackConfig.indexURL`                                                        | `getIndexPath()` / `setIndexPath()` | Not inferred at construction except for S3; inference is in static `indexFile(locator)`.                                                                                                           |
| `dbURL`                                                                                    | `getDBUrl()`                        | Legacy "database server" URL (IGV data-server style: server URL + relative path). `isLocal()` is true only when `dbURL == null` and path is not remote.                                            |
| format                                                                                     | `getFormat()` / `deriveFormat()`    | Lazily derived from filename: strips `.gz`, `.bgz`, `.txt`, `.xls`; special-cases `fpkm_tracking`, `seg.zip`, `.ewig.tdf`, `*_clusters → bedpe`, etc.; for URLs honors `?dataformat=` query param. |
| name / color / description / infoURL / labelField / visibilityWindow / filterTypes / order | via `TrackConfig`                   | Presentation config piggybacks on the locator.                                                                                                                                                     |
| `coverage`                                                                                 | `getCoverage()`                     | Optional companion density file (alignment coverage).                                                                                                                                              |
| `mappingPath`                                                                              |                                     | Variant→BAM mapping file (VCF only).                                                                                                                                                               |
| `trackLine`, `trackProperties`, `metadata`, `sampleId`                                     |                                     | UCSC track line override, parsed track properties, popup metadata.                                                                                                                                 |
| `htsget`, `dataURL`                                                                        | `isHtsget()` / `isDataURL()`        | `htsget://` scheme sets `htsget`; `data:` URIs set `dataURL`.                                                                                                                                      |

Construction paths:

- `new ResourceLocator(path)` — single constructor for files and URLs; Google Drive URLs are resolved via API to fetch the real filename.
- `ResourceLocator.getLocators(Collection<File>)` — batch: scans for index extensions `{bai, crai, sai, tbi, tbx}`; maps `foo.bam.bai → foo.bam` (and Picard-style `foo.bai → foo.bam.bai`, `foo.crai → foo.cram.crai`); attaches index to each data file.
- `ResourceLocator.fromTrackConfig(TrackConfig)` — track-hub tracks.
- `ResourceLocator.indexFile(locator)` — index inference for URLs: `.gz`/`.bgz` → `.tbi`, else tribble `.idx` (htsjdk `Tribble.STANDARD_INDEX_EXTENSION`), appended to the URL path, preserving the query string. Cloud/Dropbox URLs return null (no inference).

## 2. `org.igv.data.DataSource` — the numeric query contract

`DataSource` (interface): `getSummaryScoresForRange(chr, start, end, zoom) → List<LocusScore>`, `getDataMax/Min()`, `getDataType()`, `getWindowFunction()`/`setWindowFunction()`, `getAvailableWindowFunctions()`, `isIndexable()`, `dispose()`. Coordinates are IGV internal (0-based, start-inclusive; ends treated as exclusive bounds in binning math below).

`WindowFunction` (`org.igv.track.WindowFunction`) enum: `none, mean, median, min, max, absoluteMax, percentile2, percentile10, percentile90, percentile98, stddev, count, density`.

`AbstractDataSource` implements the generic summarization pipeline; subclasses supply raw/precomputed tiles:

- `getRawData(chr, start, end) → DataTile` — per-base/feature data (array-of-arrays, see `DataTile`).
- `getPrecomputedSummaryScores(chr, start, end, zoom) → List<LocusScore>` — zoomed tiles if the format has them (TDF, BigWig).
- Query algorithm (`getSummaryScoresForRange`): if `windowFunction != none`, try precomputed zoom tiles; otherwise compute summary tiles on the fly. Virtual tile scheme: at zoom `z` there are **2^z tiles per chromosome, 700 bins per tile**; `tileWidth = chrLength / 2^z`. Tiles are cached in an `LRUCache(10)` keyed `chr_z_tileNumber_windowFunction`; switching window function clears the cache.
- `computeSummaryTile` uses `org.igv.tdf.Accumulator` per bin: mean is a base-weighted sum (`sum += nBases*v` divided by `basesCovered` at finish); min/max/absoluteMax are running folds; median/percentiles/stddev/count/density accumulate values in a `DownsampledDoubleArrayList` (capped at 100k samples) and finish with Apache Commons `StatUtils.percentile`. Features spanning multiple bins are emitted as raw `NamedScore`s; sub-bin features are folded into a `CompositeScore` (which also keeps up to 5 representative values + probe names for popup text).
- `ORDERED_WINDOW_FUNCTIONS` defines the UI's ordered stats menu.

Implementations: `DatasetDataSource`/`CoverageDataSource` (in-memory datasets), `TDFDataSource` (precomputed TDF tiles), `BBDataSource` (BigWig zoom levels — maps IGV zoom to bases-per-pixel and picks the closest zoom level; only `min/mean/max/none` available since BigWig stores just min/validCount/sum/sumSq per zoom item), `CombinedDataSource` (multi-track overlay).

## 3. `Dataset` / `IGVDataset` — in-memory columnar data

`Dataset` (interface) is the in-memory representation for non-indexed numeric formats (`.igv`, `.cn`, GCT, expression): `getChromosomes()`, `getTrackNames()` (sample columns), `getStartLocations(chr)/getEndLocations(chr)/getFeatureNames(chr) → int[]/String[]`, `getData(trackName, chr) → float[]`, `getDataMin/Max()`, `isLogNormalized()`, `getLongestFeature(chr)`. Explicitly array-based to minimize memory ("use of objects is eschewed in favor of simple arrays").

`IGVDataset`:

- `String[] dataHeadings` — the sample/track columns.
- `Map<String, ChromosomeSummary>` — scan-time index: chromosome name, approximate data-point count (progress %), and **byte start offset in the file** so `ChromosomeData` can be lazily loaded by seeking (the parser does a first pass `scan()` recording offsets, then fetches per-chromosome on demand).
- `ChromosomeData` (LRU `ObjectCache(30)` keyed by chr): parallel arrays `int[] startLocations`, `int[] endLocations`, `String[] probes`, and `Map<String, float[]> data` keyed by column heading.
- `GenomeSummaryData` — whole-genome ("all chromosomes") summary for the genome overview.
- `IGVDatasetParser` requires files sorted by start position (throws "File is not sorted, .igv and .cn files must be sorted"); `.wig` has a parallel `WiggleDataset`/`WiggleParser`.
- `DatasetDataSource` wraps a `Dataset` as a `DataSource` (computing summaries on the fly via `AbstractDataSource`).

`DataTile` (shared raw-tile container): parallel `int[] startLocations`, `int[] endLocations`, `float[] values`, `String[] featureNames`. `SummaryTile` is just an `int startLocation` + `List<LocusScore>`.

## 4. TDF — tiled data format (`org.igv.tdf`)

IGV's own precomputed binary format; historically "IBF" (both magics accepted). Produced by igvtools (`org.igv.tools.Preprocessor`) from wig/cn/GCT inputs.

### File layout (little-endian throughout; strings are null-terminated ASCII)

```
[Header]
  magic "TDF4" (4 bytes; older "TDF"/"IBF" accepted)          int32 version (writer emits 4)
  int64 indexPosition, int32 indexByteCount  (backpatched at close)
  int32 headerByteCount
  int32 nWindowFunctions; window-function name strings         (v>=2)
  string dataType (org.igv.track.DataType); string trackLine (padded to 1024 bytes)
  int32 nTracks; track name strings
  string genomeId; int32 flags  (v>2)   — flag bit 0x1 = GZIP_FLAG (tile compression on)
[Master Index]  (nDatasets then nGroups, each entry: string name, int64 position, int32 byteCount)
[Datasets / Groups / Tiles ... arbitrary order, addressed by index]
```

- **Group** (`TDFGroup` extends `TDFEntity`): just a name + string key/value `attributes` (e.g. genome, `graphType`, autoscaling options). Used for `chromosomes`, root `/`, per-chromosome metadata.
- **Dataset** (`TDFDataset`): one per chromosome/zoom/window-function, named `/<chr>/z<zoom>/<WindowFunction>` (zoom 0 = raw; `<chr>/z0` with no WF suffix for whole-genome in v1). Fields: string dataType (`BYTE…STRING`, writer uses FLOAT), float `tileWidth`, int32 `nTiles`, then per tile: int64 file position + int32 byte size. **Position −1 marks an empty tile.** Tile selection is an implied linear index: `tileNumber = position / tileWidth` (see TODO in source noting it wants a general interval index).
- **Tiles** (`TDFTile`, types `fixedStep | variableStep | bed | bedWithName`, dispatched by `TileFactory`):
  - `TDFFixedTile`: int32 size, int32 tileStart, int32 count, float span, then `float[data][i]` per track — positions implied by `tileStart + i*span`.
  - `TDFVaryTile`: tileStart, float span, int32 nPositions, int32[] starts, then float[][] data — end = start + span.
  - `TDFBedTile`: tileStart, count, int[] starts, int[] ends, then data (+ name strings for `bedWithName`).
- **Compression:** whole-tile gzip via `CompressionUtils` when header flag set (reader decompresses before `TileFactory`).
- **Attributes** (`IBFAttributes`): int32 count, then (string key, string value) pairs — TDF's mini metadata system at header/group/dataset level.

### igvtools production (`org.igv.tools.Preprocessor`)

Streams sorted input (wig/cn parsers implement `DataConsumer`); maintains a `Raw` dataset per chromosome (tileWidth default from tooling, e.g. 16000-ish bins region) plus a `Zoom` stack of **default 7 zoom levels** (`nZoom=7`); zoom level z has **2^z tiles per chromosome** (`tileWidth = chrLength/2^z + 1`), each level holding **one dataset per window function** — default set is mean, median, min, max, absoluteMax, percentile2/10/90/98 (`allDataFunctions`). Whole-genome level uses a special `CHR_ALL` zoom-0 dataset in kb units. Tiles are closed (written) as the stream moves past them; percentiles computed with the same `Accumulator` machinery.

Reader side (`TDFReader`): opens a `SeekableStream`, reads the 24-byte pre-header, then header, master index into `Map<String,IndexEntry>`; caches datasets and groups in `LRUCache(20)` each, tiles per-dataset in `LRUCache(20)`. `getDataset(chr, zoom, wf)` builds the dataset name and fetches tiles by position — random access with no full-file scan. `LinearIndex` (`IBFIndex`) exists but is an unimplemented stub.

## 5. Tribble layer (`org.igv.feature.tribble`)

- **Codec-per-format:** `CodecFactory.getCodec(locator, genome)` switches on `locator.getFormat()` and returns an htsjdk `FeatureCodec`, mostly wrapped: `VCFWrapperCodec(VCFCodec|VCF3Codec)`, `BCF2WrapperCodec(BCF2Codec)`, `IGVBEDCodec` (bed, junctions, bedmethyl, gappedpeak variants), `GFFCodec` (GFF3/GTF/GVF versions), `PSLCodec`, `EncodePeakCodec` (narrowPeak/broadPeak/regionPeak), `UCSCSnpCodec`, `DGVCodec`, `REPMaskCodec`, `EQTLCodec`, `FPKMTrackingCodec`, `DSICodec`, `PAFCodec`, `IntervalListCodec`, `UCSCGeneTableCodec` (refflat/genepred/ucscgene/genepredext), `MUTCodec`, etc. Wrappers adapt htsjdk features to IGV features and apply genome alias handling / filter types.
- **Reader abstraction:** `IGVFeatureReader` is IGV's own minimal interface (`query(chr, start, end) → Iterator<Feature>`, `iterator()`, `getSequenceNames()`, `getHeader()`), replacing direct use of tribble's `FeatureReader`. `TribbleReaderWrapper` adapts htsjdk `AbstractFeatureReader`: note the **coordinate translation — IGV passes 0-based start, wrapper queries tribble with `start + 1`** (tribble queries are 1-based inclusive), then re-filters the iterator (`f.getStart() > end` break, `f.getEnd() < start` skip) because tribble indexes over-return.
- **Source selection (`org.igv.track.TribbleFeatureSource.getFeatureSource`):** resolves the index (explicit `locator.getIndexPath()`, else `ResourceLocator.indexFile()` inference), checks existence (remote too); for local uncompressed files > 100 MB without an index it prompts the user to create one (`IndexCreatorDialog`, igvtools index with `LINEAR_BIN_SIZE = 16000` for linear indexes, `INTERVAL_SIZE = 1000` for interval-tree indexes); > 1 GB makes an index required. Then either `IndexedFeatureSource` (queries delegated to the tribble index) or `NonIndexedFeatureSource` (loads **all features into memory**: `Map<String, List<Feature>>` per chromosome, sorted, plus a `CoverageDataSource` computed from the features and a feature database for search).
- **Index types (htsjdk):** `.idx` linear index (fixed bins of 16 kb via igvtools, htsjdk `LinearIndex` — chromosome → bin → file block offsets) vs `.idx` interval-tree index (`IntervalTreeIndex`, over 1 kb features); tabix `.tbi`/`.tbx` over bgzf-compressed files (`.gz`, `.bgz`) — all read through the same `AbstractFeatureReader` path, with bgzf block decompression handled by htsjdk over the seekable stream. `IGVComponentMethods` customizes htsjdk's codec/reader hooks (e.g. sequence-directory resolution).
- **Feature window sizing:** `estimateFeatureWindowSize` samples 1000 features, measures memory-per-feature at runtime, and computes a maximum query window that keeps a full-window load under ~20 MB (floor 1 Mb; VCF fixed at 10 kb). Above this window, tracks render summary/coverage instead of loading features.

`org.igv.util.index` contains IGV's own generic `Interval`/`IntervalTree` (red-black style interval tree used for in-memory interval queries, e.g. in feature caches) — independent of htsjdk's index classes.

## 6. UCSC binary formats (`org.igv.ucsc.bb`, plus `org.igv.ucsc.BPTree`)

`BBFile` reads both BigWig and BigBed (magics `0x888FFC26` = 2291137574 BigWig, `0x8789F2EB` = 2273964779 BigBed; byte order auto-detected from magic). The file structure comment in `BBFile` is a complete spec:

| Section            | Contents                                                                                                                                                                                                                                                                                                                    |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Header (64 bytes)  | magic, version(2B), nZoomLevels(2B), chromTreeOffset, fullDataOffset, fullIndexOffset (each 8B), fieldCount/definedFieldCount (2B each; 0 for bigWig), autoSqlOffset, totalSummaryOffset, uncompressBufSize (0 = uncompressed), extensionOffset; extended header adds `extraIndexCount` + offsets for extra (name) indexes. |
| Zoom headers ×n    | reductionLevel(4B), reserved, dataOffset, indexOffset — one R+ tree + data block per level.                                                                                                                                                                                                                                 |
| autoSql            | zero-terminated `.as` schema string (drives `BBCodecFactory` field decoding).                                                                                                                                                                                                                                               |
| Total summary      | basesCovered, minVal, maxVal, sumData, sumSquared (8B each).                                                                                                                                                                                                                                                                |
| Chromosome B+ tree | `ChromTree` over `BPTree`: key = chrom name, value = 8-byte record with chromId + chrom size.                                                                                                                                                                                                                               |
| Full data          | int64 item/section count, then sections; per-type codecs in `bb/codecs` (`BBCodec`, `BBCodecFactory`) decode bed fields from autoSql. BigWig sections are `WigDatum` items (wig sections with step/interval types).                                                                                                         |
| Full index         | R+ tree (`RPTree`) over the data sections.                                                                                                                                                                                                                                                                                  |
| Zoom levels        | each: int32 count, then records `{chromId, chromStart, chromEnd, validCount, minVal, maxVal, sumData, sumSquares}` (4B each, floats for the last three) + its own R+ tree index. This is the standard "reduction" pyramid: each level reduces ~4×, reduction level = bases per zoom item.                                   |

- **BPTree** (`org.igv.ucsc.BPTree`): Kent B+ tree, signature `0x78CA8C91` (byte order detected by swap). Header: block size, key size, val size (4B each), item count (8B), reserved. Nodes: 1B isLeaf, 1B reserved, 2B count, then items — index nodes `(key, int64 childOffset)`, leaf nodes `(key, valSize bytes)`. Keys are fixed-length, most-significant-byte-first strings. Used for the chromosome list in BigBed/BigWig **and** for name-lookup extra indexes in BigBed (`_searchTrees: BPTree[]`, e.g. gene-name search backed by `org.igv.ucsc.Trix` trix index for fuzzy name matching). Node cache per tree (`HashMap<Long, Node>`).
- **RPTree** (R+ tree, `org.igv.ucsc.bb.RPTree`): magic 610839776; 48-byte header `{blockSize, itemCount, startChromIx, startBase, endChromIx, endBase, endFileOffset, itemsPerSlot, reserved}`. Nodes: 1B type (1 = leaf), 1B reserved, 2B count; leaf items 32 bytes `{startChrom, startBase, endChrom, endBase, dataOffset, dataSize}`, child items 24 bytes (no size). **Coordinates are 0-based half-open** per UCSC spec; the overlap test is `(chrIdx2 > startChrom || (== && endBase >= startBase)) && (chrIdx1 < endChrom || (== && startBase <= endBase))`. Recursive walk with per-node cache; leaf items give (offset,size) ranges for range-request fetching, decompressed if `uncompressBufSize > 0`.
- Consumers: `BBFeatureSource` (bigBed → features via codec) and `BBDataSource extends AbstractDataSource` (bigWig; picks the zoom level matching the requested bases-per-pixel, exposes only min/mean/max/none, keeps whole-genome scores per window function).

## 7. htsget (`org.igv.htsget`)

- `ResourceLocator` recognizes `htsget://` URIs; `HtsgetUtils` parses `format`, endpoint URL, and query params from the URI.
- `HtsgetReader` is a deliberately low-level helper: for `class=header` and for region queries (`format=VCF&referenceName=<chr>&start=<s>&end=<e>`) it GETs the JSON ticket and concatenates the returned `urls` — handling inline `data:` payloads and remote URLs. **Coordinates: `readData` is called with `start + 1`** (htsget is 1-based) in `HtsgetVariantSource`.
- `HtsgetVariantSource implements FeatureSource`: only VCF is supported via this path ("htsjdk is used for BAM and CRAM which does not use this helper") — BAM/CRAM access goes through htsjdk's htsget-enabled `SeekableStream` (`SeekableServiceStream` wraps IGV's generic "range webservice" for byte ranges) rather than this JSON ticket flow. Response bytes may be BGZF-blocked (UMCCR server quirk); a `BlockCompressedInputStream` is applied when the payload starts with gzip magic `1F 8B`.
- What it replaces locally: index files entirely — the server does the subsetting; there is no `.tbi`/`.bai` and no random-access index on the client. Chrom alias mapping is derived from the VCF header contig lines. Feature window size default 1000.

## 8. Caching and remote access

- **Stream abstraction:** everything reads through `htsjdk.samtools.seekablestream.SeekableStream` obtained from `IGVSeekableStreamFactory` (`util.stream`): `IGVSeekableFileStream`, `IGVSeekableHTTPStream`, `IGVSeekableFTPStream`, `SeekableServiceStream` (range webservice / htsget-BAM), plus `SeekableSplitStream` and `IGVSeekableBufferedStream`. This is what makes local and remote access path-identical: TDF, tribble, and BigWig/BigBed all issue the same `seek/read` calls.
- **HTTP range requests:** `IGVSeekableHTTPStream._read` issues `Range: position–endRange` (endRange inclusive, clamped to content length when known), retries socket exceptions up to 3 times, and handles 416 (`UnsatisfiableRangeException`). `IGVSeekableBufferedStream` wraps streams with a **512,000-byte default buffer**, reusing the buffer when a subsequent read overlaps it — the main mechanism keeping remote reads efficient (one range request per ~500 kb, not per htsjdk block).
- **Caches are all small fixed-size LRUs, not memory-budgeted:** `LRUCache` (synchronized `LinkedHashMap` with `removeEldestEntry`) is used at size 10 (summary tiles in `AbstractDataSource`), 20 (TDF datasets, groups, per-dataset tiles); `ObjectCache(30)` for `ChromosomeData`; `RuntimeUtils.getAvailableMemory()` is consulted only heuristically, for the tribble feature-window estimate. Whole-genome summary scores in `BBDataSource` are cached per window function. Genome sequence caching lives in the genome layer, not here.
- **Search first / seek never scan:** every indexed format can answer `query(chr,start,end)` with a handful of range requests (index block → data blocks). The only full-load paths are non-indexed tribble files and `IGVDataset` formats, which are sorted-and-offset-scanned (per-chromosome byte offsets in `ChromosomeSummary`).

## Mab notes

- The **seekable-stream + (index, data) pair** abstraction is the single most transferable idea: a Rust `SeekableStream` trait (file, HTTP with Range + retry + 416 handling, buffered ~512 KB) with all formats coded against it gives local/remote parity for free. Consider async (reqwest) with a read-through buffer sized like IGV's 500 KB.
- A **codec/trait-per-format with a registry keyed by detected format** (extension + magic sniffing) mirrors `CodecFactory`; in Rust this maps naturally to `enum Format` + `Box<dyn FeatureReader>`.
- The unified numeric query trait (`DataSource`) with precomputed-zoom-first, compute-on-demand fallback, virtual tiles (2^z per chromosome, 700 bins), and a small LRU of summary tiles is a clean design worth copying; percentiles need a streaming/reservoir estimator (IGV downsamples to ≤100k values, then exact percentile).
- TDF is simple and attractive as a Mab precomputed format: little-endian, single master index, per-dataset tile tables with −1 = empty tile, per-window-function zoom datasets, gzip tile compression. Its weakness (admitted in source TODOs) is the implied linear tile index — use a real interval index instead.
- BigWig/BigBed parsing needs care with byte-order detection via magic, unsigned byte handling, and 0-based half-open coordinates (vs. tribble's 1-based query convention — IGV itself had to translate; pick one internal convention and adapt at each boundary, and document it).
- The zoom pyramid (BigWig ~4× reductions storing min/max/sum/sumSq/validCount; TDF 2^z tiles × window functions) is what makes whole-genome views cheap; keep mean computable from sumData/validCount and consider stddev from sumSquares.
- IGV's caching is primitive (fixed-size LRUs, no global memory pressure accounting). A Rust version could do better with a single size-bounded (byte-based) cache keyed by (source, tile/zoom) shared across tracks.
- htsget: IGV implements it minimally (VCF tickets; BAM via stream range service). If Mab supports it, treat it as a third query backend behind the same trait rather than a separate code path; note the 1-based query convention and BGZF-payload wrinkle IGV works around.
- Ambiguities/caveats found in source: `LinearIndex` in the tdf package is an unimplemented stub; TDF writer versions vary (v4 written, v1 files lack window functions and genome id); `TDFWriter` header reserves a fixed 1024-byte track line; percentiles rely on downsampling (approximate); the whole-genome TDF dataset uses kb units (chr length divided by 1000), a convention easy to miss.
