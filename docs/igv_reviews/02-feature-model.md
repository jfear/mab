# 02 — The Feature/Annotation Model (IGV v3.0.0-beta.4)

TL;DR: IGV's annotation model is a flat `IGVFeature` interface over 0-based, end-exclusive
intervals, with `BasicFeature` as the universal concrete carrier (attributes map, score,
color, optional exon list). GFF/GTF records are decoded individually by a tribble codec,
then a _Combiner_ rebuilds parent/child graphs (gene → transcript → exon/CDS/UTR) into the
single-feature-with-exons representation. Name search is a separate in-memory sorted map
(`FeatureDB`), not part of the interval index (see storage chapter).

All paths below are repo-relative to `src/main/java/org/igv/`.

## 1. Interface hierarchy

```
htsjdk.tribble.Feature            (getChr/getContig, getStart, getEnd — 0-based, end-exclusive)
 └─ org.igv.feature.PackedFeature (setPackedRow/getPackedRow — row assignment for "packed"/sashimi layout)
 └─ org.igv.feature.LocusScore    (+ getScore, getValueString(position, mouseX, windowFunction),
 │                                  getLengthOnReference)
     └─ org.igv.feature.IGVNamedFeature extends htsjdk.tribble.NamedFeature
         └─ org.igv.feature.IGVFeature extends LocusScore, IGVNamedFeature, PackedFeature
```

`org.igv.feature.IGVFeature` — the interface for features rendered by `FeatureTrack`s
(all `default` methods unless noted):

| Member                                                                            | Notes                                                      |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `getIdentifier()`                                                                 | stable ID (GFF `ID`, GTF `transcript_id`…); default `null` |
| `getStrand()`                                                                     | `Strand` enum; default `NONE`                              |
| `getLength()`                                                                     | `end - start`                                              |
| `getAttributeKeys() / getAttribute(key) / removeAttribute(key) / getAttributes()` | free-form key→value strings; default empty                 |
| `contains(IGVFeature)`                                                            | same chr & strand, full containment                        |
| `contains(double location)`                                                       | `start <= loc < end` (0-based half-open)                   |
| `getExons()`                                                                      | `List<Exon>` or `null`                                     |
| `getColor()`                                                                      | only abstract method                                       |
| `getURL()`, `setPackedRow/getPackedRow`, `getDisplayName(property)`               | —                                                          |

`getDisplayName(property)` resolves display name via attributes: `AbstractFeature` returns
`getAttribute(property)` falling back to `getName()`; `BasicFeature` special-cases
`property == "id"` → identifier. This is how `#displayName gene_id,gene_name` header tags
(`GFFCodec.readHeaderLine`) control what the UI labels show.

`org.igv.feature.Strand` — enum `POSITIVE("+"), NEGATIVE("-"), NONE("")`, with
`fromString` accepting `+`/`-`/`POSITIVE`/`NEGATIVE` (anything else → `NONE`).

`org.igv.feature.FeatureType` — closed enum of coarse track-level types: `OTHER, GENE,
PROMOTER, MISC_RNA, REPEAT_REGION, LTR, MUTATION, BED, GAPPED_PEAK, SPLICE_JUNCTION,
BED_METHYL, INTERACT, BEDPE, WIG, RMSK, NARROW_PEAK, PEAK`. Note this is _not_ the
per-feature `type` string (see below) which stays a free-form SO term.

`org.igv.feature.SequenceOntology` — static `Set<String>` buckets of SO-ish type strings
used by the GFF combiners:

- `exonTypes`: `exon`, `coding_exon`
- `cdsTypes`: `CDS`, `cds`
- `fivePrimeUTRTypes`: `five_prime_UTR`, `5'-UTR`, `5'-utr`, `5UTR`
- `threePrimeUTRTypes`: `three_prime_UTR`, `3'-UTR`, `3'-utr`, `3UTR`
- `utrTypes` = 5' ∪ 3' ∪ `UTR` (Ensembl GTF)
- `mrnaParts` = utr ∪ cds ∪ exon ∪ {`intron`, `polyA_sequence`, `polyA_site`, `start_codon`, `stop_codon`}
- `transcriptParts` = mrnaParts; `geneParts` = transcriptParts + `transcript`,
  `processed_transcript`, `mrna`, `mRNA`
- `isCoding(type)` = cdsTypes or `coding_exon`

## 2. `AbstractFeature` and `BasicFeature`

`org.igv.feature.AbstractFeature` — field inventory (implements `IGVFeature`):

| Field          | Type / default                              | Notes                                                                                 |
| -------------- | ------------------------------------------- | ------------------------------------------------------------------------------------- |
| `chr`          | `String`                                    | chromosome/contig name (canonicalized via `Genome.getCanonicalChrName` at parse time) |
| `start`        | `int` = -1                                  | **0-based, inclusive**                                                                |
| `end`          | `int` = -1                                  | **0-based, exclusive**                                                                |
| `strand`       | `Strand.NONE`                               | —                                                                                     |
| `type`         | `String` = ""                               | free-form (SO term for GFF; parser-specific for BED etc.)                             |
| `color`        | `Color` (nullable)                          | optional per-feature color                                                            |
| `description`  | `String` (nullable)                         | `getDescription()` falls back to `getName()`                                          |
| `attributes`   | `Map<String,String>` (lazy `LinkedHashMap`) | insertion-ordered; values are Strings                                                 |
| `name`         | `String` = ""                               | display name                                                                          |
| `packedRow`    | `int` = 0                                   | row index when features are "packed" into lanes                                       |
| `readingFrame` | `int` = -1                                  | 0-based frame offset from feature start; only meaningful for Exon                     |

Key methods: `overlaps(Feature)` = `end >= o.start && start <= o.end && chr.equals` (note:
uses `>=`/`<=` on the half-open coordinates, so touching features "overlap" — a slight
asymmetry with `IGVFeature.contains(double)` which is `start <= loc < end`).
`getLocusString()` converts to human 1-based: `chr + ":" + (start+1) + "-" + end`.
`setColor(String[] rgb, int nTokens)` accepts 1 (gray) or 3 RGB tokens.

`org.igv.feature.BasicFeature extends AbstractFeature` — adds:

| Field                     | Type / default          | Notes                                                                                        |
| ------------------------- | ----------------------- | -------------------------------------------------------------------------------------------- |
| `representation`          | `String`                | optional raw rendering hint                                                                  |
| `exons`                   | `List<Exon>` (nullable) | child exon structures                                                                        |
| `score`                   | `float` = `NaN`         | e.g. BED score; `NaN` = absent                                                               |
| `confidence`              | `float`                 | copied by copy-constructor                                                                   |
| `identifier`              | `String`                | stable ID distinct from display `name`                                                       |
| `thickStart` / `thickEnd` | `int`                   | UCSC-style coding interval; ctor clamps to start/end, and `setStart/setEnd` keep them inside |
| `parentIds`               | `String[]`              | GFF `Parent` values (comma-split; multiple parents supported)                                |
| `link`                    | `String`                | external URL                                                                                 |

Behavior worth noting:

- `addExon(Exon)` grows the feature bounds to the exon envelope (min start / max end).
- `addUTRorCDS(BasicFeature)`: if a `BasicFeature` (CDS/UTR record from GFF) is contained
  in an existing exon, that exon absorbs the coding bounds (`codingStart/codingEnd`,
  `readingFrame`, `nonCoding` flag) and the exon's attributes are _replaced_ by the CDS
  attributes; otherwise a new `Exon` is created.
- `sortExons()` sorts by start.
- Coordinate-conversion helpers (used by HGVS/mutation code):
  - `featureToGenomePosition(int[] featurePositions)` — transcript (mRNA) 0-based offsets → 0-based genomic positions, walking exons from the 5' side; `-1` for unmappable.
  - `genomeToCodingPosition(int)` — 0-based genomic → 0-based CDS offset.
  - `codingToGenomePosition(int codingPosition)` — **1-based** coding position → 0-based genomic.
  - `getCodon(Genome, chr, proteinPosition)` — protein position (1-based) → `Codon` with the 3 genomic bases, using `CodonTableManager.getCodonTableForChromosome`.

`org.igv.feature.GFFFeature extends BasicFeature` — thin subclass adding
`componentAttributes: List<String>`: human-readable dumps of column-9 attributes of each
merged child (exon/CDS/UTR) shown in popup text via `mergeAttributes`.

`org.igv.feature.Exon extends AbstractFeature implements IExon` — the sub-feature model
for splicing:

| Field                       | Notes                                                                                                                       |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `number`                    | exon index relative to 5' end, 1-based (set by combiners; negative-strand transcripts are numbered from `size()` downwards) |
| `codingStart` / `codingEnd` | 0-based, half-open coding sub-interval clamped to exon bounds (`setCodingStart/End` clamp); default = whole exon            |
| `noncoding`                 | flag; `setNonCoding(true)` collapses codingStart=codingEnd to the strand-appropriate edge                                   |
| `mrnaBase`                  | 0-based offset of exon's first coding base within the mRNA (-1 if unset); set externally                                    |
| `readingFrame` (inherited)  | see §4 phase handling                                                                                                       |
| `aminoAcidSequence`         | lazily computed `AminoAcidSequence` cache, invalidated if the codon table changes                                           |

Key methods: `getCodingLength()` = `noncoding ? 0 : max(0, codingEnd - codingStart)`;
`getAminoAcidNumber(int genomeCoordinate)` (**0-based** genomic) → 1-based aa number using
`mrnaBase` and strand; `getAminoAcidSequence(genome, prevExon, nextExon)` translates the
coding region, borrowing bases from the adjacent exons when `readingFrame > 0` (split
codons across exon junctions). `Exon.getExonProxy(IExon)` is a Java dynamic-proxy trick to
give location-based equality/hashCode — irrelevant to Rust; use a struct key instead.

`org.igv.feature.IExon` — tiny interface: start/end/cdStart/cdEnd/chr/strand accessors.

## 3. Attributes

- Storage is per-feature: `AbstractFeature.attributes`, a `LinkedHashMap<String,String>`
  (lazy; `setAttribute` creates it). No global registry, no schema, no typed values.
- GFF column 9 maps directly: each `key=value` pair becomes one entry; repeated keys keep
  the _last_ value; a key with no value maps to `""`. Order is preserved (LinkedHashMap).
- There is no `AttributeMap` type in this codebase; the attribute-related shared code is
  `org.igv.feature.FormatUtils.printAttributes` (HTML popup rendering) and
  `org.igv.track.AttributeManager` / `AttributeSupplier` (UI-side: which attribute columns
  to display in track headers — display concern, not model).
- **Standard attribute constants** (de facto, encoded in `GFFCodec`'s helpers, not an enum):
  - GFF3: `ID`, `Name`, `Alias`, `Parent` (comma-separated multi-parent).
  - GFF2/GTF parent candidates tried in order: `transcript_id`, `id`, `mRNA`,
    `systematic_id`, `gene`, `transcriptId`, `Parent`, `proteinId`.
  - Name fields, GFF2 default: `alias, gene, ID, Locus, locus, Name, name, gene_name,
    primary_name, systematic_id, transcript_id, gene_id`; GFF3 default: `Name, Alias, ID,
    gene, locus, gene_name`. Overridable with `#displayName=k1,k2` header lines.
  - Color: keys `color|Color|colour|Colour` parsed into `Color` (`ColorUtilities.stringToColor`).
  - `Gene Ontology` style tags are not specially handled; everything unknown stays in
    the attributes map verbatim.
- `FeatureDB` indexes _all_ attribute values shorter than 50 chars as search keys (§5).

## 4. Parser hierarchy

Two parallel mechanisms:

1. `org.igv.feature.FeatureParser` (interface: `loadFeatures(BufferedReader, Genome)` +
   `getTrackProperties()`) with `AbstractFeatureParser` base (handles `#track`/
   `#coords`/`#gffTags`/`#type` header lines, collects features into a list, registers them
   in `Genome.getFeatureDB()`). Concrete legacy parsers exist (e.g. `GFFParser`,
   deprecated) but the modern path is:
2. `org.igv.feature.tribble` codecs: `AbstractFeatureParser.getInstanceFor()` uses
   `CodecFactory.getCodec()` to get an `AsciiFeatureCodec` and wraps it in
   `org.igv.feature.FeatureCodecParser`. Codecs (one per format, all producing `BasicFeature`
   or subclasses): `GFFCodec`, `IGVBEDCodec`, `UCSCCodec`/`UCSCGeneTableCodec`,
   `REPMaskCodec`, `EncodePeakCodec`, `MUTCodec`, `IntervalListCodec`, `PAFCodec`,
   `DGVCodec`, `EMBLTableCodec`, `UCSCSnpCodec`, `VCFWrapperCodec`, `BCF2WrapperCodec`.
   The tribble/indexed I/O mechanics are covered by the storage chapter.

### `GFFCodec` (`org.igv.feature.tribble.GFFCodec`)

- `enum Version { GFF2, GFF3, GTF }`; version auto-detected from `##gff-version 3`
  (default GFF2 helper until seen). `canDecode` by extension: `.gff .gff3 .gvf .gtf` (+
  `.gz`).
- Header directives: `##gff-version 3` (switches to GFF3Helper), `##provider … GENCODE`
  (sets `gencode` flag), `#nodecode`/`##nodecode` (disable URL-decoding of attributes),
  `#hide type1,type2` (types decoded but dropped from output), `#displayName=fields`,
  `#track` lines → `TrackProperties`, `##FASTA` (stops decoding — embedded sequence
  section).
- Column mapping (tab-separated, requires ≥9 columns; 0-based array indices shown):
  0 seqid → chr (canonicalized); 2 type → `setType` (also used for `ignoredTypes` filter);
  3 start, 4 end; 6 strand; 7 phase; 8 attributes.
- **Coordinates:** GFF/GTF are 1-based inclusive; codec converts:
  `start = parse(tokens[3]) - 1` (rejects start < 1 with "GFF is 1-based" error), `end` kept
  as-is (it was already exclusive-equivalent). IGV-internal convention thereafter is
  0-based inclusive start / 0-based exclusive end.
- **Phase → reading frame:** for GFF3/GTF, column 8 `phase ∈ {0,1,2}` (number of bases to
  skip before the next codon) is converted to IGV's `readingFrame = (3 - phase) % 3`
  (0-based offset of the first full codon within the feature). GFF2's ambiguous frame
  column is ignored for translation.
- `ID`/`Name`/`Parent` extraction is delegated to `Helper` implementations:
  - `GFF3Helper`: `ID` = attributes["ID"]; `Parent` = attributes["Parent"].split(",");
    attributes split on `;` then `=`, URL-decoded by default.
  - `GFF2Helper`: parses space- or `=`-delimited `key value;` pairs, strips quotes;
    parent ID by probing the `possParentNames` list; ID by trying attributes[type],
    attributes[type+"_id"], then the idFields list.
- If no ID is found, one is synthesized: `"igv_" + UUID`.
- UTR records get preliminary `thickStart`/`thickEnd` hints based on strand (5'UTR on
  `+` / 3'UTR on `-` → `thickStart = end`, i.e. coding starts after the UTR; mirrored for
  the other case).

### Transcript assembly: the GFF Combiners (`org.igv.feature.gff`)

IGV's in-memory model is _one transcript = one `BasicFeature` with an ordered exon list_,
while GFF is a flat parent-child graph. `GFFCombiner` (interface:
`addFeatures(Iterator<Feature>)`, `addFeature(BasicFeature)`, `combineFeatures()`) closes
that gap; `GFFFeatureSource.getCombiner(version)` picks `GFF3Combiner` vs
`GFF2Combiner` (legacy, "not actively supported").

`GFF3Combiner.addFeature` buckets records:

| Bucket                              | Condition                                       | Fate in `combineFeatures()`                                                                                                                                                                                         |
| ----------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `gffExons`                          | type ∈ exonTypes and has parent                 | converted to `Exon(nonCoding = !isCoding(type))`, attached to each parent; missing parents auto-created from the exon's bounds with `id` as name                                                                    |
| `gffUtrs`                           | type ∈ utrTypes and has parent                  | merged into existing exons or creates new ones via `parent.addUTRorCDS(...)`                                                                                                                                        |
| `gffCdss` (`GFFCdsCltn` per parent) | type ∈ cdsTypes                                 | overlay onto exons; **two CDS conventions handled**: (1) all CDS records of an isoform share one ID → single transcript; (2) each CDS has a unique ID → each distinct CDS ID becomes its own isoform (`copyForCDS`) |
| `multiLineFeaturesMap` (per chr)    | has `ID`, colliding with another same-ID record | multi-line/discontinuous features merged: bounds unioned, parts added as exons                                                                                                                                      |
| `gffFeatures`                       | has `ID`, otherwise new                         | the transcript candidates; become final `GFFFeature`s                                                                                                                                                               |
| `igvFeatures`                       | everything else                                 | passed through unchanged                                                                                                                                                                                            |

Final pass over all assembled features: `sortExons()`; assign 5'-relative exon numbers
(descending numbering on `−` strand); recompute `thickStart` = first coding exon's
`cdStart` and `thickEnd` = last coding exon's `cdEnd`; merge child column-9 attributes
into `GFFFeature.componentAttributes`; sort the whole list with
`FeatureUtils.sortFeatureList`.

`GFF2Combiner` is identical except it lacks the multi-line merge (a same-ID GFF2 record
overwrites rather than merges) — see its class comment.

`org.igv.feature.gff.GFFFeatureSource` wraps an indexed tribble `FeatureSource` for GFF
files: for each window query it expands the interval by **±2,000,000 bp**, pulls raw
records, re-runs the combiner on the fly (because transcripts span records), then trims
results to the requested window. This is a notable cost model: GFF transcript assembly is
repeated per query window, not done once at load.

## 5. `FeatureCollection`, `FeatureCollectionSource`, `FeatureDB`

- There is **no `FeatureCollection` class** in this codebase; the equivalent is
  `org.igv.track.FeatureCollectionSource<T extends Feature>`: `Map<String chr, List<T>>`
  of features sorted by start (built in `initFeatures` via `FeatureUtils.sortFeatureList`),
  with `getFeatures(chr, start, end)` doing a **linear scan + predicate filter**
  (`FeatureUtils.getOverlapPredicate`: `chr.equals && f.start <= end && f.end > start` —
  half-open overlap on 0-based coordinates). It also computes `CoverageDataSource`
  summary scores. Explicitly documented as "a legacy implementation, and does not scale
  to large feature tracks" — real tracks use the tribble indexed path (storage chapter).
- Binary-search utilities over start-sorted lists live in
  `org.igv.feature.FeatureUtils`: `getFeatureAt(position, buffer, list)` (first feature
  whose [start−buffer, end+buffer] contains the 0-based position), `getIndexBefore`,
  `getFeatureStartsAfter`, `getFeatureCenteredAfter/Before`, `getFeatureClosest`
  (tie-break: distance → |pos − start| → shortest feature), `getOverlapPredicate`.
- `org.igv.feature.FeatureDB` — the **name index** backing the search box (populated by
  parsers via `genome.getFeatureDB().addFeatures(...)`):
  - `TreeMap<String, List<NamedFeature>>` (case-folded keys, uppercase; sorted-map prefix
    scans for autocomplete: `subMap(prefix, prefix + Character.MAX_VALUE)`).
  - Keys inserted per feature: `name` (skip null/empty/"."), `identifier` (BasicFeature),
    every attribute value < 50 chars, and the same for all exons.
  - Each key maps to a capped list (`MAX_DUPLICATE_COUNT = 20`) sorted by
    `FeatureComparator`: prefer shortest chromosome-name length, then by feature length
    (direction configurable; used so e.g. "chr1" beats "chr1_random").
  - `getFeature(name)` → first match; `getFeaturesMatching(name)` → exact matches with
    fallback to querying `Track.isSearchable()` tracks (results cached under the query);
    `getFeaturesStartingWith(name, limit)` for text hints.

## 6. Coordinate conventions (summary)

| Context                                                  | Convention                                                                                |
| -------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Internal (`Feature`/`BasicFeature`/`Exon`, BED, tribble) | 0-based start **inclusive**, end **exclusive**; `contains(loc)` = `start <= loc < end`    |
| GFF/GTF/PSL input                                        | 1-based inclusive; codecs subtract 1 from start at parse time                             |
| `Locus` / locus strings (`chr:1-100`)                    | 1-based inclusive both ends; `Locus.fromString` does `start = parsed - 1`, `end = parsed` |
| `Exon.codingStart/codingEnd`                             | 0-based half-open, clamped within exon                                                    |
| `readingFrame`                                           | 0-based offset from feature start; derived from GFF phase as `(3 − phase) % 3`            |
| Protein positions (`getCodon`, amino-acid numbering)     | 1-based                                                                                   |
| `#coords startBase` header (AbstractFeatureParser)       | lets a legacy file declare a non-zero base offset; default 0                              |

## 7. Cyto and base-pair and aa features (brief)

- `org.igv.feature.Cytoband implements IGVNamedFeature` — fields `chromosome, name, start,
  end` (0-based, per the parser's use of `Cytoband` with UCSC cytoBand.txt), `type: char`
  (`p` = gpos from gieStain string, `c` = acen/centromere), `stain: short` (0–100, default
  100 when absent). Parsed by `org.igv.feature.CytoBandFileParser` into
  `LinkedHashMap<String chr, List<Cytoband>>` (file order preserved); rendered by
  `org.igv.feature.cyto.CytobandTrack`. Not `IGVFeature`s — no strand/score/attributes.
- `org.igv.feature.basepair.BasePairFeature implements htsjdk.tribble.Feature` —
  intra-chromosomal base-pair/loop contacts: `startLeft, startRight, endLeft, endRight`
  (the feature's overall span is `startLeft..endRight`), plus `colorIndex`. Backed by
  `BasePairFileParser`/`BasePairData` and drawn by `BasePairTrack` as arcs.
- `org.igv.feature.aa` — translation machinery, not genomic features:
  - `AminoAcid` (enum-like with `NULL_AMINO_ACID`), `AminoAcidManager` (singleton,
    3-letter/1-letter mappings), `AminoAcidSequence` (strand, 0-based genomic `start`,
    `List<CodonAA>`, plus the `codonTableKey` id used to invalidate cached translations).
  - `CodonTable` (immutable; built from a JSON resource: `id`, `names`, `starts`,
    `codonMap: Map<String,AminoAcid>`, `altStartCodons`) and `CodonTableManager`
    (per-chromosome codon table selection, NCBI translation tables from classpath
    resources).
  - `Codon` (proteinPosition 1-based; three genomic positions in 0-based UCSC style,
    filled in reverse order on `−` strand; computes its own sequence from `Genome`).
  - `CodonAA` — one residue plus its genomic coordinates (used in `AminoAcidSequence`).

## 8. Cross-references

- Indexed file access, `.idx`/tribble index structure, `FeatureReader` wiring:
  see the storage chapter (`feature/tribble` internals are intentionally not covered here).
- Rendering of these features (`FeatureTrack`, `RenderedFeature`, `PackedFeature` rows):
  tracks chapter.

## Mab notes

- Core trait: `Feature { chr, start: u32 (0-based incl), end: u32 (excl), strand: Option<Strand>,
  type: String, name, identifier: Option<String>, score: Option<f32>, color: Option<Color>,
  attributes: indexmap (ordered, string→string), parent_ids: Vec<String> }`. An ordered map
  (`IndexMap`) reproduces IGV's `LinkedHashMap` popup ordering.
- Represent transcripts as a struct with `Vec<Exon>` rather than a generic parent/child
  graph — IGV flattens the GFF graph at parse time and never needs the graph again.
  Do the flatten _once_ at load (IGV's per-window re-assembly in `GFFFeatureSource` is a
  known inefficiency).
- GFF→model translation needs: SO term buckets (exon/CDS/UTR sets with the tolerant
  alias lists), phase→frame conversion `(3 − phase) % 3`, the two CDS-ID conventions, and
  per-chr multi-line feature merging. Copy `SequenceOntology`'s string sets verbatim.
- Attribute→display-name resolution (`#displayName`-style key priority lists) is cheap and
  very useful; adopt a `name_fields: &[&str]` priority list per format.
- Keep the name index (`FeatureDB`) separate from the interval index: a `BTreeMap`-style
  sorted map over uppercase names with capped duplicate lists gives exact match + prefix
  autocomplete. Index name + identifier + short attribute values.
- The internal 0-based half-open convention everywhere, with explicit conversions at the
  format boundary, is worth replicating; encode it in newtypes (`Position`, `Interval`) so
  1-based formats can't leak in.
- UTR/CDS overlay semantics (`addUTRorCDS`: CDS attrs replace exon attrs) and thickStart/
  thickEnd recomputation are subtle; write golden tests against IGV's behavior for a few
  real GENCODE/Ensembl files.
- Ambiguity noted: `AbstractFeature.overlaps` treats touching intervals as overlapping;
  `IGVFeature.contains(double)` is half-open. Pick one overlap definition and use it
  consistently in Rust.
