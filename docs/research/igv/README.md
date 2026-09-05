# IGV — Deep Architectural Reviews

Agent-friendly reference documentation distilled from the Integrative Genomics
Viewer (IGV) source code, for use while designing and building Mab. The focus
is on **concrete structures, attributes, and storage mechanics** — the parts
most useful for designing Mab's own data model in Rust. Java/Swing
implementation specifics are kept brief; each doc ends with Mab notes flagging
what transfers.

## Package map

All packages are under `src/main/java/org/igv/` (a tiny legacy
`org.broad.igv.ui.Main` entry point remains):

```
org.igv
├── feature               Feature/annotation model (Feature, BasicFeature, Exon,
│   │                     FeatureUtils, FeatureDB, parsers, Locus/Range)
│   ├── .genome           Genome, Chromosome, Sequence backends, GenomeManager,
│   │   ├── .fasta        aliases (ChromAlias*), cytobands, GenomeConfig/loaders
│   │   └── .load         Indexed/block-compressed FASTA
│   ├── .gff              GFF/GTF transcript combiners
│   ├── .aa, .basepair,   translation machinery; base-pair arcs; cytoband track
│   │   .cyto
│   ├── .bionano,         SMAP, DRanger, EMBL-table codecs
│   │   .dranger, .embl
│   └── .tribble          htsjdk codec-per-format layer + index wiring
├── alignment             Alignment model (Alignment, SAMAlignment, blocks/gaps,
│   ├── .cram             AlignmentTrack, coverage, packing, downsampling)
│   ├── .mods             CRAM reference source + disk cache
│   ├── .reader           base modifications (MM/ML tags)
│   ├── .smrt, .sbx       reader factory (SAM/BAM/CRAM/lists)
├── track                 Track abstraction (Track, TrackProperties, TrackLoader,
├── renderer              TrackGroup, AttributeManager) + Renderers, DataRange
├── data                  DataSource/Dataset/DataTile, coverage, window functions
├── tdf                   TDF/IBF precomputed binary format
├── ucsc                  BigWig/BigBed (bb/), 2bit (twobit/), BPTree, hubs, Trix
├── variant, vcf          VCF/variant track + source
├── session, prefs        Session (JSON/XML), autosave, PreferencesManager
├── batch                 Command language, BatchRunner, TCP CommandListener
├── event                 IGVEventBus + event types
├── ui                    IGV singleton, MainPanel/TrackPanel/DataPanel,
│   └── .svg              ReferenceFrame, locus parsing, snapshots
├── circview, lists       circular view; gene lists
├── htsget                htsget ticket reader (VCF)
├── maf, bedpe, seg,      multiple alignment, interactions, segmentation,
│   gwas, sashimi,        GWAS, splice-junction plots, Hi-C, repeats,
│   hic, repeats, encode  ENCODE peak support
├── charts, sample,       charts, sample/attribute tables, igvtools,
│   tools, util           utilities (ResourceLocator, index, liftover, streams)
└── ...
```

## Sources

Everything here was derived from the **IGV v3.0.0-beta.4** source distribution
(paths below are relative to the repository root):

| Source                    | Path                         | Used for                                                |
| ------------------------- | ---------------------------- | ------------------------------------------------------- |
| IGV Java source (Java 21) | `src/main/java/`             | All type/structure details, verified against the code   |
| Subsystem map             | `CLAUDE.md`                  | Package orientation                                     |
| Test sessions             | `test/sessions/`             | Session XML/JSON schema examples                        |
| Build                     | Gradle, single project `igv` | Key dependency: **htsjdk 5.0.0** (BAM/CRAM/VCF/tribble) |

- IGV version: **v3.0.0-beta.4** (git tag); packages mostly `org.igv.*`.
- ~921 `.java` files under `src/main/java`.

## Document index

| Doc                                                    | Contents                                                                                                                                                                                        |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [01-genome-model.md](01-genome-model.md)               | `Genome`, `GenomeManager`, `Chromosome`, sequence backends (indexed/bgzf FASTA, 2bit), `SequenceWrapper` tile cache, chromosome aliases, cytobands, `GenomeConfig`, whole-genome view, liftover |
| [02-feature-model.md](02-feature-model.md)             | `Feature`/`BasicFeature` field inventory, attributes, GFF/GTF combiners, `Exon`, `FeatureDB` name index, coordinate conventions per format                                                      |
| [03-track-render.md](03-track-render.md)               | `Track` interface + state, full `TrackProperties` catalog, `TrackLoader` format dispatch, `DataRange`, renderer hierarchy, color cascades, `AttributeManager`, visibility/display modes         |
| [04-alignment-model.md](04-alignment-model.md)         | `Alignment`/`AlignmentBlock`, htsjdk boundary, CIGAR→blocks/gaps, loading pipeline + caching, row packing, coverage arrays, downsampling, base modifications, `AlignmentTrack` options          |
| [05-storage-query.md](05-storage-query.md)             | `ResourceLocator`, `DataSource` query contract, TDF on-disk layout, tribble codecs + indexes, BigWig/BigBed BPTree/RPTree, htsget, seekable streams + HTTP range caching                        |
| [06-session-prefs-batch.md](06-session-prefs-batch.md) | Session JSON/XML structure, autosave, preference system + key catalog, batch command language, TCP/REST command listener, `IGVEventBus`                                                         |
| [07-ui-viewer.md](07-ui-viewer.md)                     | Panel hierarchy, `ReferenceFrame` viewport math, locus parsing + search pipeline, gene lists, regions of interest, circview, SVG/PNG capture; Swing-specific vs transferable                    |
| [08-io-formats.md](08-io-formats.md)                   | Master format table (parser/index/queryability/coordinates for every supported format), format auto-detection, exporters, coordinate-conversion summary                                         |

## How to use these docs

- **Designing a Mab data structure** (genome, features, alignments): read
  01–04. They describe the _shape_ of IGV's data, which is the best guide for
  Mab's Rust types.
- **Designing Mab storage/persistence**: read 05 (plus the sequence backends
  in 01 and the session format in 06).
- **Designing Mab's track/render layer**: read 03 (and 07 for the viewport).
- **Adding a file format**: read 08 first (format → codec/index/conventions),
  then the chapter owning that format's model.
- **Designing Mab's UI**: read 07; most of IGV's UI is Java/Swing-specific
  and only the _concepts_ (viewport math, panel layout, navigation events)
  transfer to Iced.

## Conventions used in these docs

- Type names are IGV names, package-qualified on first mention per section
  (e.g. `org.igv.track.Track`). Paths are repo-relative; packages are
  abbreviated (`feature/genome` = `org.igv.feature.genome`).
- Coordinates: IGV's internal convention is **0-based, start-inclusive /
  end-exclusive** everywhere; 1-based formats (GFF, VCF, seg, GWAS, locus
  strings, …) are converted at parse time. Each doc states conventions
  explicitly wherever they appear.
- Each doc ends with a **Mab notes** section: short implications for the Rust
  implementation. These are opinions/starting points, not decisions.
- Ambiguities found in the source are noted in-line rather than papered over.

## Big-picture takeaways (structure focus)

1. **One internal coordinate system, converted at the edge.** Every parser
   normalizes to 0-based half-open immediately; rendering and querying never
   think about input conventions. The most transferable pattern in the whole
   codebase.
2. **Tiny metadata types + pluggable backends.** `Chromosome` is
   name/index/length; sequence, aliases, and cytobands are separate lazy
   sources on `Genome`. `Feature` is a flat record with an ordered attribute
   map and an optional exon list — GFF graphs are flattened at parse time.
3. **Codec-per-format behind one query trait.** Text formats go through a
   tribble codec registry (`CodecFactory`); numeric formats share one
   `DataSource` contract (precomputed-zoom-first, compute-on-demand fallback,
   window functions). Binary formats (BAM/CRAM via htsjdk, BigWig/BigBed,
   TDF, 2bit) have dedicated readers over a common seekable-stream
   abstraction, so local files and HTTP range requests share code paths.
4. **Per-window derived data, cached as a unit.** Alignments load into an
   `AlignmentInterval` carrying reads + coverage arrays + junctions +
   downsample intervals + packed rows, evicted when out of view. Coverage is
   struct-of-arrays; downsampling is windowed reservoir sampling that keeps
   mates together.
5. **Track = state container; renderers read it.** All visual configuration
   (`TrackProperties`, `DataRange`, colors, display mode, visibility window)
   lives on the track; renderers are parameterized objects. Sessions
   serialize exactly this state (JSON in IGV 3), and tracks self-serialize
   via a `Persistable` contract.
6. **Multi-resolution everywhere.** Whole-genome view is a kbp concatenation
   of long chromosomes; numeric data uses zoom pyramids (TDF: 2^z tiles ×
   window functions; BigWig: ~4× reductions with min/max/sum/sumSq); tracks
   switch feature→coverage rendering via visibility windows.
7. **Flat, scriptable control surface.** A line-oriented batch command
   language (load/genome/goto/region/snapshot/…) over files and a TCP socket
   doubles as the igv.js integration protocol — headless control for free.
