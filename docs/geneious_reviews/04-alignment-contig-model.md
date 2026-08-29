# 04 — Alignments, Contigs & Assemblies

Packages: `...publicapi.documents.sequence`
(`SequenceAlignmentDocument`, `AlignmentLayout`),
`...publicapi.implementations` (`DefaultAlignmentDocument`,
`EndGapsManager`, `PairedReadManager`), `...publicapi.plugin`
(`SequenceSelection`).

## Key fact: alignment ≡ contig

> "The only difference between an alignment or contig (`isContig()`) is how
> they are displayed to the user. At the code level they are identical."

Both are `SequenceAlignmentDocument`s; a contig may additionally have a
reference sequence row.

---

## SequenceAlignmentDocument (abstract)

`public abstract class SequenceAlignmentDocument implements PluginDocument,
PairedReads`. Only `getNumberOfSequences()` and `getSequence(int)` are
abstract — everything else has defaults for read-only alignments.

### Structure / accessors

| Member                                                                                        | Notes                                                                                                                        |
| --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `getNumberOfSequences()`, `getSequence(i)`, `getSequences()`, `getSequencesInImmutableList()` | Rows are `SequenceDocument`s (gapped)                                                                                        |
| `isContig()`                                                                                  | Display mode only                                                                                                            |
| `getContigReferenceSequenceIndex()`                                                           | Index of reference row, or -1                                                                                                |
| `getCircularLength()`                                                                         | Gapped length at which a circular alignment wraps; 0 = linear                                                                |
| `getAnnotationsOnConsensus()`                                                                 | Annotations on the consensus sequence                                                                                        |
| `getMateIndex(i)`, `getMateExpectedDistance(i)`, `getImmutablePairedReadManager()`            | Paired reads ([02](02-sequence-model.md#paired-reads))                                                                       |
| `getReferencedDocument(i)` / `getReferencedSequence(i)` (+ list variants)                     | The original document/region each row was aligned from (`ReferencedSequence` carries the source document + extracted region) |
| `asJeblAlignment(silentlyStripDuplicates)`, `getSequenceAsJeblSequence(i)`                    | Bridge to the JEBL phylogenetics library                                                                                     |
| `getRealignedName(name)` / `getRefinedName(name)`                                             | Static naming conventions for derived alignments                                                                             |

### Editing capability flags (capability-style API)

`isEditable()`, `canAddAndRemoveSequences()`, `canSetAnnotations()`,
`canSetAnnotationsOnConsensus()`, `canSetSequenceNames()`,
`canSetFieldValue(sequenceIndex, field)`; mutation methods include
`addSequence(seq)`, `setAnnotations(rowIndex, annotations, ...)`,
`setAnnotationsOnConsensus(...)`, `updateSequence(rowIndex, newSequence,
annotations, nucleotideGraph, ...)`, `setFieldValue(rowIndex, field, value,
...)`.

### ReferencedSequence (nested)

Represents "row _i_ was aligned from document X, region Y" — the basis for:

- Propagating annotation edits back to source sequences.
- Track gap-inserting views ([03](03-annotations-tracks.md#sequencetrack)).
- Re-extracting/changing the source.

---

## DefaultAlignmentDocument (in-memory implementation)

`public class DefaultAlignmentDocument extends SequenceAlignmentDocument` —
the standard implementation for alignments that fit in memory. Also
implements: `SequenceListSummary.Provider`,
`PluginDocument.SizeRequiredToLoadIntoMemoryProvider`,
`SequenceTrack.Manager.Provider`, `Renamable`,
`XMLSerializableWithProgress`.

### Constructors

- `(name, alignedSequences...)` — no references to originals.
- `(unalignedSequences, referencedDocuments, alignedChars, options, score,
  proposedName)` — **the usual constructor**: keeps ungapped originals +
  references + gapped characters.
- `(unalignedSequences, List<ReferencedSequence>, alignedChars, options,
  score, proposedName, progress)` — region-aware references.
- `(AnnotatedPluginDocument[] referencedDocs, alignedChars, options, score)`
- Copy ctor from any `SequenceAlignmentDocument`; empty ctor.
- `fromJeblAlignment(Alignment)` — from JEBL.

### Notable members

- Contig building: `setContig(boolean)`, `setContigReferenceSequenceIndex(i)`,
  `addContigReferenceSequence(seq)`.
- Mates: `setMates(i, j, expectedDistance)` (see `ExampleAssembler`).
- Score: `KEY_ALIGNMENT_OPTIONS`, `KEY_ALIGNMENT_SCORE`, `KEY_MATCH_REGIONS`;
  static `calculateScore(scores, alignment, gapOpen, gapExtend, freeEndGaps,
  applyGapExtendCostToFirstGapResidue)`.
- Large-alignment bridge: `getAlignmentDataForSequencesNotInMemory()` →
  `SequenceListOnDisk.AlignmentData` when non-reference rows live on disk;
  provides `getSummary()`, pre-built `EndGapsManager`, and `AlignmentLayout`s.
- Consensus: annotations on consensus, `setConsensusSequence(...)`, etc.

---

## Scale structures for big contigs

### SequenceListOnDisk.AlignmentData

Immutable bundle: the non-reference sequences of an alignment (as a
`SequenceListOnDisk`) + associated data (end-gaps index, layouts). Obtained
from `DefaultAlignmentDocument.getAlignmentDataForSequencesNotInMemory()`.
Built via `SequenceListOnDisk.Builder`
([02](02-sequence-model.md#sequencelistondisk-lazy-list--the-scale-workhorse)).

### EndGapsManager (column → covering-rows index)

`public class EndGapsManager` — answers "which sequences cover alignment
column _c_?" quickly, excluding rows that are in end-gap territory at _c_.
The reference sequence counts like any read. Core acceleration structure for
contig rendering and coverage computation.

- Construction: from an alignment or `SequenceCharSequence[]` +
  `minimumHashSize`; `EndGapsManager.Builder` for too-big-for-memory builds;
  copy-with-reference ctor adds/replaces a reference at index 0; deserialize
  from XML + the associated `SequenceListOnDisk`.
- Queries:
  - `getSequencesCoveringArray/Iterable/Iterator(residueIndex)` — exact set.
  - `getPotentialSequencesCoveringArray(residueIndex[, circularLength])` —
    superset (may include non-covering), cheaper.
  - `getSamePotentialSequencesLowerBound(i)` / `...UpperBoundExclusive(i)` —
    column range over which the potential set is unchanged (render runs of
    columns in one go).
  - `getHashSize()`, `getNumberOfColumns()`, `getSequenceCount()`,
    `getSequence(i)`, `getSequences()`.
- Persistence: `toXmlExcludingSequences(...)` — the index is stored without
  the sequences; reconstructed with the sequence list supplied separately.

### AlignmentLayout (row assignment for rendering)

`public final class AlignmentLayout` — assigns each non-reference sequence to
a display row so rows never overlap (minimum separation), optionally forcing
mates into the same row. Data is **never all in memory** — loaded on demand
with caching (`getMaximumColumnsCached()`). Three flavors per contig:
`getLinkAllPairsLayout()`, `getLinkNearbyPairsLayout()`,
`getLinkNoPairsLayout()`. Constructed as part of
`SequenceListOnDisk.Builder`; don't construct directly.

---

## Consensus, coverage, quality binning (contig metadata)

Contigs carry computed metadata as document fields
([01](01-document-model.md#standard-field-catalog)) rather than dedicated
API: `CONTIG_MEAN_COVERAGE`, `CONSENSUS_SEQUENCE_LENGTH`,
`CONSENSUS_SOURCE_SEQUENCES_FIELD`, `BIN` / `BIN_REASON` (Low/Medium/High
quality bins for chromatograms and contigs), `BINNING_FRAME` /
`BINNING_GENETIC_CODE` (frame/code minimizing stop codons),
`CONTIG_PERCENTAGE_OF_REFERENCE_SEQUENCE_COVERED`,
`CONTIG_REFERENCE_SEQUENCE_INDEX/LENGTH`, `REFERENCE_SEQUENCE_NAME`.

---

## CombinedAlignmentAndSequenceDocument

`public abstract class CombinedAlignmentAndSequenceDocument extends
SequenceAlignmentDocument implements EditableSequenceDocument` — one object
that is **both** a standalone sequence and an alignment of parts of it.

- Use case: BLAST-style search hits — the hit behaves as a sequence for
  downstream operations, but the viewer can show the pairwise alignment; once
  the full document downloads, both views are available.
- Variants: `CombinedAlignmentAndNucleotideSequenceDocument`,
  `CombinedAlignmentAndAminoAcidSequenceDocument`,
  `CombinedAlignmentAndNucleotideGraphSequenceDocument`.
- Doesn't implement the track provider; use
  `SequenceTrack.getTrackManager(...)` helpers.
- Selection indices into combined docs are disambiguated by
  `SequenceSelection.SequenceDocumentType` (see below).

---

## SequenceSelection (viewer selection model)

`public final class SequenceSelection implements XMLSerializable`
(`plugin` package) — the user's current selection in the sequence/alignment
viewer; passed into operations/annotation generators and broadcast between
viewers.

### Structure

- One or more `SelectionInterval`s:
  - `getResidueInterval()` → `Interval` (0-based residue range)
  - `getSequencesRange()` → range of selected sequence indices
- `SequenceIndex` — which sequence: index relative to the **first sequence of
  the first selected document** (flattened across all selected documents),
  plus a `SequenceDocumentType` (`Sequence` vs `Alignment`) to disambiguate
  combined documents. `getSequence()` resolves to the actual document.
- `SelectedAnnotation` — a selected annotation.
- `SequenceSelectionWithDocuments` — selection pinned to specific documents,
  with `ExtractionStrategy` and exceptions
  (`DocumentsInvalidException`, `SelectedDocumentsModifiedException`) for
  stale selections.

Usage patterns seen in examples: iterate
`selection.getIntervals(false)` → per interval iterate
`selectionInterval.getSequencesRange()` and clip residue ranges to
non-gap content for performance.

---

## Assembly input/output model (for context)

Full assembler contract in [07](07-operations-options.md#assemblers);
data-shape summary:

- Input (`AssemblerInput`): libraries of reads (each a sequence list or
  alignment document), optional reference sequences, data types
  (`AssemblerInput.DataType` — sequencing technology), paired data types
  (`PairedDataType`), flags (generate contigs, save used/unused reads,
  fraction of data to use), max contigs limit.
- Reads iterate **once** via `AssemblerInput.Reads` (`hasNext()`,
  `getNextReadPair()` → `Read` with `getReadNormalized()` /
  `getMateNormalized()` / `getExpectedMateDistanceNormalized()`), so the
  assembler streams rather than materializes.
- Output via `Assembler.Callback`: `addContigDocument(contig, ...)`,
  `addUnusedRead(read, progress)` / `addUnusedReads(list, progress)`,
  consensus variants. Contigs are `DefaultAlignmentDocument`s with
  `setContig(true)`, reference index and mates set.

---

## Mab notes

- Modeling alignment and contig as one type with a display flag is pragmatic;
  Mab can keep the data model unified and differentiate views.
- The reference-row convention (index 0, special handling everywhere) is
  simple but pervasive; an explicit `reference: Option<RowIndex>` may be
  cleaner in Rust.
- The three scale structures — lazy row storage (`AlignmentData`),
  column→rows index (`EndGapsManager`), and row-layout cache
  (`AlignmentLayout`) — are exactly what a Rust contig viewer will need.
  Note each is persisted _separately from the sequences_ and rebuilt lazily;
  they are indexes, not content.
- Coverage/consensus metadata lives as plain document fields, not first-class
  structures — cheap to extend, weakly typed. Mab can decide per attribute.
- The flattened selection model (document-list-relative indices + type tag)
  handles combined documents; an enum of selection targets might express this
  more safely in Rust.
