# 03 — Annotations and Tracks

Packages: `...publicapi.documents.sequence` (`SequenceAnnotation`,
intervals, qualifiers), `...publicapi.implementations.sequence`
(`SequenceTrack`), `...publicapi.implementations` (gap translation).

Annotations ("features" in GenBank terminology) describe regions of
sequences. They live in two places:

1. **Directly on the sequence** — `SequenceDocument.getSequenceAnnotations()`.
2. **On tracks** — named, separately-managed annotation lists attached to a
   sequence (`SequenceTrack`), see below.

Coordinate convention: annotation coordinates are **1-based, inclusive**;
core sequence indices elsewhere are 0-based.

---

## SequenceAnnotation

`public final class SequenceAnnotation implements XMLSerializable`.

### Structure

| Attribute  | Accessor          | Notes                                                                                             |
| ---------- | ----------------- | ------------------------------------------------------------------------------------------------- |
| Name       | `getName()`       | Human label                                                                                       |
| Type       | `getType()`       | String, usually one of the `TYPE_*` constants                                                     |
| Intervals  | `getIntervals()`  | `List<SequenceAnnotationInterval>`; multi-interval features (e.g. split genes) are one annotation |
| Qualifiers | `getQualifiers()` | `List<SequenceAnnotationQualifier>` key/value metadata                                            |

Mutation API (annotations are mutable objects): `addInterval(...)` (three
forms incl. `(from, to)` left-to-right), `addIntervals(Collection)`,
`addQualifier(name, value)` / `addQualifier(SequenceAnnotationQualifier)` /
`addQualifiers(...)`.

### Static helpers / special behaviors

- `copyOf(List<SequenceAnnotation>)` — deep copy.
- `copyWithoutIntervals()` — copy stripped of intervals.
- `createMaskedAnnotation(min, max)` → `TYPE_MASKED`;
  `createTrimAnnotation(min, max)` → `TYPE_TRIMMED`.
- `containsOnPotentiallyCircularSequence(start, length, lengthIfCircular)` —
  circular-aware containment.
- Invisible annotations: any type prefixed with
  `INVISIBLE_ANNOTATION_TYPE_PREFIX` is hidden in the viewer.
- Active-link bookkeeping: `HIDDEN_TYPE_FOR_ACTIVELY_LINKED_PARENTS`,
  `createHiddenAnnotationForDeletingThisFromActivelyLinkedChild(URN)`,
  `applyHiddenAnnotationsForDeletingFromActivelyLinkedChild(...)` — used so
  edits to an actively-linked parent can remove matching annotations on
  regenerated children.
- Hidden alignment storage: `getAlignmentHiddenQualifierName(...)` /
  `getAlignmentsFromHiddenQualifiers()` — alignments can be smuggled inside
  hidden qualifiers of single sequences (used by sequence search hits).

---

## SequenceAnnotationInterval

`public final class SequenceAnnotationInterval` — immutable directed range;
**both ends inclusive**, 1-based. May extend outside the sequence range for
partial features.

### Structure

- `getFrom()` (start), `getMaximumIndex()`, `getMinimumIndex()`,
  `getLength()` (residues + gaps).
- `getDirection()` → `Direction` enum:
  `leftToRight`, `rightToLeft`, `none`, `bothWays`
  (`isDirectedLeft/Right()`, `reverse()`, `toArrowString()`).
- Truncation flags: `isTruncatedAtMinimumIndex()` /
  `isTruncatedAtMaximumIndex()` (partial features at sequence edges).
- Gap-restoration adjustments: `minAdjustmentWhenRestoringGaps`,
  `maxAdjustmentWhenRestoringGaps` + truncation flags for the same —
  keeps intervals stable when gaps are removed and later re-added.
- Between-bases: zero-length intervals via `between(min, max[,
  gapLengthToExpandToFill])`; `isBetweenBases()`. The optional expand-to-fill
  length makes a between-bases annotation stretch across a gap column of that
  size in alignments.

### Constructors & conversions

- `(from, to)` — left-to-right, inclusive.
- `(min, max, direction[, truncatedMin, truncatedMax[, gapRestore args...]])`.
- Copy ctor; from `Interval` (0-based, exclusive-max); from string form
  `"[...]n,nnn D m,mmm[...]"` (comma-grouped numbers, arrow direction).
- `asInterval()` — convert to 0-based exclusive `Interval`.
- `adjustIntervalForGapInsertion(SequenceGapInformation)` /
  `adjustIntervalForGapRemoval(...)` — coordinate remapping.

---

## Annotation type catalog (`SequenceAnnotation.TYPE_*`)

String constants; groupings below are editorial.

**Genes & transcripts:** `TYPE_GENE`, `TYPE_CDS`, `TYPE_MRNA`, `TYPE_NCRNA`,
`TYPE_RRNA`, `TYPE_TRNA`, `TYPE_TMRNA`, `TYPE_MATURE_PEPTIDE`,
`TYPE_SIGNAL_PEPTIDE`, `TYPE_EXON`, `TYPE_INTRON`, `TYPE_PRECURSOR_RNA`,
`TYPE_TRANSLATED_REGION`, `TYPE_ORF`.

**Regulatory & binding:** `TYPE_PROMOTER`, `TYPE_TERMINATOR`,
`TYPE_PRIMER_BIND`, `TYPE_PRIMER_BIND_REVERSE`, `TYPE_PROTEIN_BIND`,
`TYPE_DNA_PROBE`, `TYPE_REGULATORY`, `TYPE_ORIGIN_OF_REPLICATION`,
`TYPE_ORIGIN`.

**Variation & differences:** `TYPE_CONFLICT`, `TYPE_MISC_DIFFERENCE`,
`TYPE_EDITING_HISTORY_DELETION`, `TYPE_EDITING_HISTORY_INSERTION`,
`TYPE_EDITING_HISTORY_REPLACEMENT` (directionless records of edits vs. the
originally imported sequence), `TYPE_HIGH_COVERAGE`, `TYPE_LOW_COVERAGE`.

**Structure (protein):** `TYPE_HELIX`, `TYPE_STRAND`, `TYPE_TURN`,
`TYPE_COIL`, `TYPE_COILED_COIL`, `TYPE_DISULFIDE_BOND`, `TYPE_CHAIN`,
`TYPE_TRANSMEMBRANE`, `TYPE_DOMAIN`.

**Cloning/assembly bookkeeping:** `TYPE_CLONING_FRAGMENT` (source fragment
provenance in cloning results), `TYPE_INSERTED_SEQUENCE`, `TYPE_LIGATION`,
`TYPE_OVERHANG` (terminal single-strand overhang), `TYPE_GIBSON_PRIMER_EXTENSION`,
`TYPE_EXCLUDED_REGION` (length-0 region excluded from a view),
`TYPE_EXTRACTED_REGION` (invisible index-mapping annotation),
`TYPE_CONCATENATED_SEQUENCE` (deprecated → `TYPE_CLONING_FRAGMENT`).

**Sequencing/assembly:** `TYPE_TRIMMED` (replaces deprecated
`TYPE_LOW_QUALITY`), `TYPE_JUNCTION` (large deletions/introns from reference
mapping or SAM/BAM import), `TYPE_MASKED` (phylogeny masking),
`TYPE_SEARCH_HIT`, `TYPE_STATISTICS`, `TYPE_WIG` (wiggle/coverage data).

**Repeats & misc:** `TYPE_MOTIF`, `TYPE_REPEAT_REGION`, `TYPE_REPEAT_UNIT`,
`TYPE_LONG_TERMINAL_REPEAT`, `TYPE_SPACER`, `TYPE_CRISPR`,
`TYPE_RESTRICTION_SITE`, `TYPE_POTENTIAL_RESTRICTION_SITE`, `TYPE_SITE`,
`TYPE_SOURCE`, `TYPE_MISC_FEATURE`, `TYPE_MISC_RNA`,
`TYPE_EXPRESSION_LEVEL`, `TYPE_EXPRESSION_DIFFERENCE`, `TYPE_OPTIMIZED_CODON`,
`TYPE_VECTOR_STRONG`, `TYPE_VECTOR_MODERATE`, `TYPE_VECTOR_WEAK`,
`TYPE_VECTOR_SEGMENT_OF_SUSPECT_ORIGIN`.

---

## SequenceAnnotationQualifier

`public final class SequenceAnnotationQualifier` — immutable string
name/value pair shown in the annotation hover popup.

Standard/special qualifier names:

- `COLOR` — override annotation color;
  `HIDDEN_QUALIFIER_BACKGROUND_COLOR` — background override.
- `HIDDEN_PREFIX` — qualifier names starting with this are hidden from UI.
- `HIDDEN_QUALIFIER_EXCLUDE_FROM_ANNOTATION_RESULT_COUNTER` — don't count as
  a generator result.
- `HIDDEN_QUALIFIER_FOR_ACTIVELY_LINKED_PARENTS` — active-link bookkeeping.
- `EDITING_HISTORY_ORIGINAL_BASES` — original bases under an editing-history
  annotation.
- `GENETIC_CODE_NAME` — genetic code specification.
- `ALL_LOCATIONS` — display all matching locations for repeat annotations.
- `EXTRACTED_REGION_INTERVALS_NAME` — value is a parsable
  `SequenceAnnotationInterval` list (index mapping);
  `EXTRACTED_REGION_ORIGINAL_CIRCULAR_SEQUENCE_LENGTH`.
- Expression data: `EXPRESSION_FPKM`, `EXPRESSION_RPKM`, `EXPRESSION_TPM`,
  `EXPRESSION_RAW_READ_COUNT`, `EXPRESSION_RAW_FRAGMENT_COUNT`,
  `EXPRESSION_RAW_TRANSCRIPT_COUNT`.
- Differential expression: `DIFFERENTIAL_EXPRESSION_LOG2_RATIO_QUALIFIER`,
  `..._P_VALUE`, `..._ADJUSTED_P_VALUE`, `..._BASE_MEAN`, `..._WALD_STAT`,
  `..._LOG_FOLD_STANDARD_ERROR_QUALIFIER`, `..._CONFIDENCE_QUALIFIER`,
  `..._ABSOLUTE_CONFIDENCE_QUALIFIER`, `..._METHOD`, `..._RATIO_QUALIFIER`.

---

## SequenceTrack

`public class SequenceTrack` (`implementations.sequence`) — a named list of
annotations attached to a sequence **in addition to** the sequence's own
annotations.

### Structure & behavior

- Tracks have: name, annotations, track-level qualifiers
  (`getQualifiers()`), and optional associated **data files**
  (`addTrackDataFile(identifier, File)` — a copy is stored with the track).
- Standard qualifier: `DESCRIPTION_QUALIFIER`; `NO_TRACK` is the display name
  for untracked annotations.
- `ANNOTATION_TYPES_NEVER_ON_TRACKS` — types that must stay directly on the
  sequence for the viewers to work: `TRIMMED`, `EDITING_HISTORY_*`,
  `OVERHANG`, `LIGATION`, `CONCATENATED_SEQUENCE`, `CLONING_FRAGMENT`,
  `EXTRACTED_REGION`, `TRANSLATED_REGION`.
- **Lazy loading:** when a track manager loads, all track _metadata_ comes
  into memory, but each track's annotations load on demand via
  `getSequenceAnnotations(progress)`.
- Multi-sequence tracks: there is no cross-sequence track object; in sequence
  lists the viewer groups tracks **by name** into one apparent track, and
  name/qualifier edits apply to all same-named tracks.
  `Manager.getUniqueTrackName(managers, prefix)` creates a spanning unique
  name.
- Alignments: track data is **not duplicated** into alignments that reference
  ungapped sequences — alignments fetch tracks from the referenced sequence
  and insert gaps on demand via `Manager.createGapInsertingManager(...)`, so
  a track annotation at ungapped 2..4 appears as gapped 2..5, and edits
  propagate back to the source sequence. Alignments can also provide tracks
  on their consensus sequence.
- After any track change, the document still needs
  `AnnotatedPluginDocument.saveDocument()`.

### Manager

`SequenceTrack.Manager` — all tracks on one sequence; add/edit/delete tracks.
Documents implement `SequenceTrack.Manager.Provider`; obtain managers via
`SequenceTrack.getTrackManager(sequence)`. Constructors:
`new SequenceTrack(name)` or deserialize from XML.

---

## Gap-coordinate translation (annotations ↔ alignments)

`SequenceGapInformation` (`implementations`) — precomputed gap map for a
gapped `CharSequence`; **0-based** indices; translates indices beyond
sequence ends too (annotations can overshoot). ~0.5 bytes/base for large
sequences. Some reference sequences in big contigs carry a pre-built instance
(`DefaultSequenceDocument.getSequenceGapInformation()`,
`SequenceGapInformation.Provider`).

Translation API:

- `getGappedIndex(ungappedIndex)` /
  `getUngappedIndexOfThisOrPreviousResidue(gappedIndex)` (+
  `...OrNextResidue...`, + `...TreatingEndGapsLikeInternalGaps` variants).
- `getGappedCharAt`, `getGappedCharSequence`, lengths, leading/trailing gaps.
- `adjustAnnotationsForGapInsertion(annotations)` /
  `adjustAnnotationsForGapRemoval(annotations)` — batch remap.
- `SequenceAnnotationInterval.adjustIntervalForGapInsertion/Removal(...)` —
  per-interval remap.
- `SequenceUtilities.createSequenceCopyAdjustedForGapInsertion(ungappedSeq,
  gappedCharSequence)` — produce the aligned copy of a sequence with all its
  annotations/tracks remapped (used by assemblers).

---

## Editing annotations

- `SequenceDocumentWithEditableAnnotations.setAnnotations(List)` — replace
  all; `EditableSequenceDocument.setSequenceAndAnnotations(seq, annotations)`
  — replace both.
- Recommended path is via a `SequenceAnnotationGenerator`
  ([07](07-operations-options.md)): the sequence viewer then handles undo/redo
  and saving automatically.
- Manual path (from the API FAQ): get envelope → get editable sequence → copy
  annotation list → add → `setAnnotations` → `document.saveDocument()`.

---

## Mab notes

- Annotations as _mutable_ objects with 1-based inclusive multi-intervals and
  direction is GenBank-shaped; if Mab keeps this, import/export fidelity is
  cheap. If Mab standardizes on 0-based half-open everywhere, provide the
  GenBank bridge explicitly.
- The "never-on-tracks" type list is a lesson about _semantic_ annotation
  types (trimming, edit history, mapping) vs. _display_ annotations — worth
  separating these concepts in Mab's type system instead of a string type +
  blacklist.
- Tracks-as-named-groups-with-lazy-content + gap-inserting views is a clean
  pattern for coverage/RNA-seq-style tracks; the per-name grouping across
  lists is a UI-level hack Mab could replace with real group ids.
- Gap translation deserves a dedicated, well-tested module in Mab; Geneious
  exposes ~10 variants of the index-translation function to handle edge-gap
  semantics — decide those semantics deliberately.
