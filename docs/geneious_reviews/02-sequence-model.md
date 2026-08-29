# 02 — Sequence Data Model

Packages: `...publicapi.documents.sequence` (interfaces + core value types),
`...publicapi.implementations.sequence` (concrete implementations).

## Type hierarchy

```
PluginDocument
└── SequenceDocument                    (one sequence)
    ├── NucleotideSequenceDocument      (marker: DNA/RNA)
    │   ├── NucleotideGraphSequenceDocument   (+ quality/chromatogram)
    │   │   ├── EditableNucleotideGraphSequenceDocument
    │   │   └── ChromatogramDocument          (deprecated marker; use instanceof NucleotideGraphSequenceDocument)
    │   └── EditableSequenceDocument ...
    └── AminoAcidSequenceDocument       (marker: protein)

SequenceDocumentWithEditableAnnotations  (annotations editable, sequence not)
└── EditableSequenceDocument             (sequence + annotations editable:
                                          setCircular, setSequenceAndAnnotations)
```

Concrete implementations must implement one of the two marker interfaces —
Geneious dispatches on them, so a bare `SequenceDocument` implementation is
not usable.

### SequenceDocument surface

| Member                                                  | Notes                                                                                                                  |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `getCharSequence()` → `SequenceCharSequence`            | Preferred access (cheap, gap-aware)                                                                                    |
| `getSequenceString()` → `String`                        | Convenience copy; avoid for large sequences                                                                            |
| `getSequenceLength()`                                   | Gapped length                                                                                                          |
| `getSequenceAnnotations()` → `List<SequenceAnnotation>` | Annotations directly on the sequence (tracks are separate, see [03](03-annotations-tracks.md))                         |
| `isCircular()`                                          | Circular topology flag                                                                                                 |
| `GENOME_SEQUENCE_THRESHOLD`                             | Length at which a sequence is treated as a _genome_ (100,000 at API 4.50): different serialization and viewer defaults |
| `SequenceDocument.Alphabet` enum                        | `PROTEIN`, `NUCLEOTIDE`                                                                                                |
| `SequenceDocument.Transformer`                          | Helper for producing transformed copies of sequences                                                                   |

### Implementations

- `DefaultSequenceDocument` (abstract base, `AbstractPluginDocument`
  subclass): editable, provides tracks (`SequenceTrack.Manager.Provider`),
  optional pre-built gap info (`SequenceGapInformation.Provider`), has a
  nested `Cache` controlling char-sequence/annotation caching. Constructors:
  `(name, description, CharSequence, Date[, URN])`, copy ctor, JEBL sequence.
- `DefaultNucleotideSequence`, `DefaultAminoAcidSequence`,
  `DefaultNucleotideGraphSequence` — the workhorse constructors used by
  importers/operations (e.g.
  `new DefaultNucleotideSequence(name, description, "GATTACA", new Date())`).
- `OligoSequenceDocument` + `OligoType` — primer/oligo sequences.
- `CombinedAlignmentAnd*SequenceDocument` — see
  [04](04-alignment-contig-model.md).

---

## SequenceCharSequence (the residue container)

`public abstract class SequenceCharSequence implements CharSequence,
XMLSerializable, Comparable<SequenceCharSequence>` — **immutable** wrapper
around another immutable `CharSequence`, adding first-class knowledge of
terminal gaps (`'-'` runs at either end). This is the central memory/scale
type of the sequence model.

### Why it exists

- Stores long runs (e.g. end gaps) without materializing characters.
- Alignment rows are ungapped content + terminal gap counts; prepending/
  appending gaps is O(1) (`withTerminalGaps(int, CharSequence, int)`).
- Prefer `CharSequence`/`SequenceCharSequence` over `String` in APIs —
  implementations can compute values on the fly and store repetitive data
  compactly, plus carry meta-information.

### Key operations and their complexity

| Operation                                                                                                                                       | Cost                                                                  | Notes                                  |
| ----------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- | -------------------------------------- |
| `getLeadingGapsLength()`, `getTrailingGapsLength()`, `getTrailingGapsStartIndex()`, `isAllGaps()`                                               | O(1)                                                                  |                                        |
| `subSequence(start, end)`                                                                                                                       | usually O(1)                                                          | not guaranteed                         |
| `getInternalCharSequence()`                                                                                                                     | O(1)                                                                  | content without terminal gaps          |
| `insert(index, seq)`, `delete(begin, end)`                                                                                                      | first call O(n), then **O(log n)** for subsequent edits on the result | Returns modified copies (immutable)    |
| `charAt`, `charAtIgnoringEndGaps`, `isGap`, `isEndGap`, `count(char)`, `contains(char)`, `countGaps(from, to)`, `indexOf(CharSequence[, from])` |                                                                       |                                        |
| `getUngappedLength()`                                                                                                                           |                                                                       | excludes all gaps, internal + terminal |
| `containsInvalidResidues(SequenceType, allowGaps)`                                                                                              | quick tri-state check (`MaybeBoolean`)                                |                                        |
| `compareTo`, `equals`, `equalsIgnoreCase`, `hashCode`                                                                                           | lexicographic / content-based                                         |                                        |

### Construction & serialization

- `valueOf(CharSequence)` — wrap any immutable char sequence (idempotent).
- `withTerminalGaps(prefixGaps, seq, suffixGaps)` — alignment row in O(1).
- `withOnlyGaps(n)`, `EMPTY`.
- XML: `toXML([allowFileData])`, `toXML(majorVersion, progress[, allowFileData])`,
  `fromXml(Element)`; guaranteed serializable down to major version 6.0.
- Binary: `writeObject(DataOutput|GeneiousObjectOutputStream, progress)` /
  `readObject(...)` — compact binary format used for on-disk sequence lists.

### Contract

The wrapped `CharSequence` must never change length or characters (esp.
terminal gaps); violations cause nondeterministic behavior or runtime
exceptions. Thread safety follows from immutability. Use
`SequenceUtilities` / `CharSequenceUtilities` which special-case this type
for speed.

---

## Quality scores and chromatograms — NucleotideGraph

`public interface NucleotideGraph` — optional phred-like quality data and
optional chromatogram trace, attached to nucleotide sequences
(`NucleotideGraphSequenceDocument`). Two independent parts:

1. **Per-residue quality**: `hasSequenceQualities()`,
   `getSequenceQuality(residueIndex)`, `getSequenceQualities(start, end)`.
2. **Chromatogram trace**: may exist for any subset of A/C/G/T
   (`hasChromatogramValues(stateNumber)`); sampled at more positions than
   called bases:
   - `getChromatogramLength()` — number of trace points (≥ sequence length)
   - `getChromatogramValue(nucleotideStateNumber, graphPosition)`
   - `getChromatogramPositionForResidue(residueIndex)` — trace position where
     each base was called; must be strictly increasing with residue index
   - `hasChromatogramPositionsForResidues()`

Design notes: 454-style data has qualities only; ABI traces have both.
`NucleotideGraph.ImmutableGraphProvider` lets graphs hand out immutable
instances (shareability). `DefaultNucleotideGraph` and
`CompactQualityOnlyGraph` are provided implementations.

---

## Paired reads

`public interface PairedReads` — implemented by all
`SequenceAlignmentDocument`s, optionally by sequence lists.

| Member                                   | Meaning                                                  |
| ---------------------------------------- | -------------------------------------------------------- |
| `getMateIndex(sequenceIndex)`            | Index of mate in the same list, or -1                    |
| `getMateExpectedDistance(sequenceIndex)` | Expected distance = read1 length + insert + read2 length |
| `getImmutablePairedReadManager()`        | Immutable `PairedReadManager` view                       |

Sign convention (unaligned): depends on library orientation —
Forward/Forward: (+, −); Forward/Reverse (Illumina short): (+, +);
Reverse/Forward (Illumina long): (−, −); Reverse/Reverse (454): (−, +).
**In a correctly oriented alignment the left read is always positive and the
right read negative** (reverse-complementing a read negates its distance).

`PairedReadManager` (`implementations`): tracks mates + distances for a whole
list; `Builder` constructs it on disk for huge datasets;
`Orientation` enum { `ForwardForward`, `ForwardReverse`, `ReverseForward`,
`ReverseReverse` } for interlaced reads; `getInterlacedExpectedDistance()` /
`getInterlacedOrientation()` summarize uniform libraries. Mutable during
assembly building (`addSequence()`, `setMates(i, j, distance)`), then frozen.

---

## ImmutableSequence (compact sequence)

`public abstract class ImmutableSequence extends SequenceCharSequence
implements SequenceDocument` — minimal-footprint sequence for large read
sets. Stores: name, bases, leading/trailing gap lengths, per-residue quality
(**no** chromatogram traces, **no** annotations/descriptions).

- Build: `createImmutableSequence(sequence[, failIfContainsNonStorableData])`
  or `ImmutableSequence.Builder` which additionally detects **common name
  patterns** across many sequences to compress names.
- Bulk compaction: `compactNucleotideSequences(list, progress)` /
  `compactAminoAcidSequences(...)` replace list entries in place.
- Fast-path accessors: `getData(internalStart, size, byte[] chars,
  byte[] quals, offset)` (batch), `getCharAndQualityAtIgnoringEndGaps(i)`
  (combined call ≈ 2× faster than separate).
- `asDna()` / `asRna()` copies; `createCopyWithNewName/Suffix`;
  `getLeadingTrimLength()` / trailing trim (materializes `TYPE_TRIMMED`
  annotations' effect without storing annotations).

---

## Sequence lists

### SequenceListDocument (interface)

Holds 0+ nucleotide and 0+ protein sequences:

- `getNucleotideSequences()`, `getAminoAcidSequences()` — the returned lists
  may themselves be `SequenceListOnDisk` or `SequenceListSummary.Provider`.
- `getSequence(index)` — unified index across both lists.
- Editable variants: `EditableSequenceListDocument`,
  `ExtendedEditableSequenceListDocument` (list must be told when a member
  sequence is edited individually).
- `DefaultSequenceListDocument` — standard in-memory implementation.

### SequenceListOnDisk (lazy list — the scale workhorse)

`public abstract class SequenceListOnDisk<T extends SequenceDocument> extends
AbstractList<T> implements SequenceListSummary.Provider, XMLSerializable` —
sequences load **from disk on demand**; LRU-ish caching so recent sequences
aren't re-read. Immutable except caching; thread-safe.

Contract highlights:

- Returned sequences must be treated as immutable even if they implement
  editable interfaces (changes die with the cache entry).
- **Iterate in order** — random access defeats sequential disk reads.
- `get(i)` failures surface as `RuntimeException` wrapping `IOException`
  (possibly wrapping `OutOfMemoryError`).
- `cacheSequencesInMemory(progress)` — preload all for hot access.
- `create(list, tryCompressingSequences, progress)` — from in-memory list;
  `createInterlaced(list1, list2, progress)` — interleave mate pairs.
- Cache policy knobs: `getMaximumSequencesCached(isAlignmentOrContig)`,
  `getMaximumCachedUnalignedSequenceLength()`.
- Serialized via `toXML(elementName)` for embedding in a PluginDocument;
  `fromXml(Element)` recreates.
- `SequenceListOnDisk.AlignmentData` — immutable bundle of alignment rows
  - associated data excluding the reference sequence (used by
    `DefaultAlignmentDocument.getAlignmentDataForSequencesNotInMemory()`).

`SequenceListOnDisk.Builder<T>` (not thread safe):

- `Builder(tryCompressingSequences, alphabet, allowGaps[, progress, maxSequences, multiThreaded])`.
- Two-pass name trick for compression: call `addNameOfSequence(name)` for all
  sequences first, then `addSequence(...)`.
- `addSequence(seq, progress)`, `addSequenceWithMate(seq, mate, dist1, dist2,
  progress)`, `addAlignmentReferenceSequence(ref, progress)`.
- Finishing: `toSequenceListDocument(...)` / `toAlignmentDocument(...)`.
- Thresholds: `getMinimumSuggestedContigSizeForCreatingContigsOnDisk()`,
  `getMinimumSuggestedReferenceLengthForCreatingContigsOnDisk()`.

### SequenceListSummary (aggregate stats without iteration)

`public final class SequenceListSummary implements XMLSerializable` —
distributions of sequence lengths, qualities, annotation types, etc., for
large lists/alignments:

- Providers: lists from `SequenceListDocument.get*Sequences()` may implement
  `SequenceListSummary.Provider`; `DefaultAlignmentDocument.getSummary` does
  when non-reference rows are on disk.
- Not thread safe while building; immutable (thread safe) after
  `finishedAddingSequences(progress)`; deserialized instances are immutable.
- Nested: `Coverage` (sequences covering an alignment column),
  `PairwiseSimilarity` (identical/non-identical pairs per column).

---

## Memory strategy summary (how Geneious handles scale)

| Data shape                        | Strategy                                                                                                                                    |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Many short reads (millions)       | `SequenceListOnDisk`: sequences lazily loaded, ordered iteration, optional compression; metadata (names, lengths, summaries) in memory      |
| Chromosome-scale single sequences | The `SequenceDocument` objects load with their list, but their `SequenceCharSequence` and `SequenceTrack` contents load **on demand** later |
| Alignment rows                    | Ungapped sequence + terminal gap counts (`SequenceCharSequence`); gaps never stored as characters                                           |
| Big contigs                       | Reference in memory; other rows via `SequenceListOnDisk.AlignmentData` + pre-built `EndGapsManager` ([04](04-alignment-contig-model.md))    |
| Read sets                         | `ImmutableSequence` (+ name-pattern compression) for minimal per-sequence overhead                                                          |
| Gap math                          | Precomputed `SequenceGapInformation` (~0.5 bytes/base for >10 Mbp gapped sequences)                                                         |
| Editing                           | Copy-on-write everywhere; immutable value types shared freely                                                                               |

Utility packages worth knowing: `SequenceUtilities`,
`CharSequenceUtilities`, `SequenceExtractionUtilities` (subsequence
extraction with `ExtractionOptions`), `NucleotideCounter`.

---

## Mab notes

- The gap model is the single most transferable idea: alignment rows as
  _ungapped content + terminal gap counts_ with O(1) end-gap operations makes
  alignment construction cheap and storage compact. In Rust this is a natural
  struct (`content: Arc<...>`, `leading: u64`, `trailing: u64`).
- Dual indexing (0-based core vs 1-based inclusive annotations) is explicit;
  whatever Mab chooses, keep it uniform and provide the same kind of
  translation helpers (`SequenceGapInformation`).
- Lazy sequence lists should be a first-class type, not a runtime trick:
  ordered iteration requirements, cache limits, and error semantics are all
  part of Geneious's API contract.
- Consider separate compact representation types for reads vs.
  chromosome-scale sequences — Geneious deliberately uses different
  strategies for each.
