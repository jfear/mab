# Geneious Prime — Deep Architectural Reviews

Agent-friendly reference documentation distilled from the Geneious Prime Plugin
Development Kit, for use while designing and building Mab. The focus is on
**concrete structures, attributes, and storage mechanics** — the parts most
useful for designing Mab's own data model in Rust.

## Package map

All API packages (`com.biomatters.geneious.publicapi.*`):

```
publicapi
├── (root)                  Geneious (version / client-mode helpers)
├── .components             Swing widgets, Dialogs, look-and-feel integration
├── .databaseservice        DatabaseService, WritableDatabaseService, Query model,
│                           QueryField, RetrieveCallback, FolderViewDocument
├── .documents              PluginDocument, AnnotatedPluginDocument, URN,
│ │                         DocumentField, notes, XMLSerializable, DocumentUtilities,
│ │                         OperationRecordDocument
│ ├── .sequence             SequenceDocument family, SequenceAnnotation*, SequenceCharSequence,
│ │                         SequenceListOnDisk, PairedReads, NucleotideGraph, AlignmentLayout
│ └── .types                TreeDocument, PublicationDocument, MolecularStructureDocument,
│                           TaxonomyDocument
├── .implementations        DefaultAlignmentDocument, EndGapsManager, PairedReadManager,
│ │                         SequenceGapInformation, tree/phylogeny implementations
│ ├── .sequence             DefaultSequenceDocument family, SequenceTrack, ImmutableSequence
│ │                         helpers, NucleotideCounter
│ └── .structure            Molecular structure implementations (PDB, CML, XYZ, ...)
├── .laf                    Look and feel
├── .plugin                 GeneiousPlugin, all extension-point types, Options,
│                           GeneiousActionOptions, PluginUtilities, TestGeneious,
│                           SequenceSelection, Findable
└── .utilities              FileUtilities, SequenceUtilities, CharSequenceUtilities,
    │                       SequenceExtractionUtilities, CallSoon, ImportUtilities
    └── .xml                XML helpers
```

## Sources

Everything here was derived from the **Geneious Prime Plugin Development Kit
2026.1.2** (the official SDK distribution; paths below are relative to its
root):

| Source                            | Path                          | Used for                                                  |
| --------------------------------- | ----------------------------- | --------------------------------------------------------- |
| Public API Javadoc (HTML)         | `api-javadoc/`                | Exact class/method/field structure and behavior contracts |
| Example plugins (17, Java source) | `examples/`                   | Concrete usage patterns of every extension point          |
| API overview + FAQ                | `api-javadoc/index.html`      | Plugin type taxonomy, common recipes                      |
| Plugin dev walkthrough (PDF)      | `PhobosPluginDevelopment.pdf` | Build/packaging workflow                                  |

- Geneious version: **2026.1.2**; public API major version: **4** (plugins
  declare `getMaximumApiVersion() == 4`, `getMinimumApiVersion()` like
  `"4.0"`/`"4.11"`).
- Java packages all live under `com.biomatters.geneious.publicapi.*`.

## Document index

| Doc                                                          | Contents                                                                                                                                                                                                                               |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [01-document-model.md](01-document-model.md)                 | `PluginDocument`, `AnnotatedPluginDocument`, `URN`, `DocumentField` (full standard-field catalog), notes, additional XML, serialization/versioning, provenance records, custom document types                                          |
| [02-sequence-model.md](02-sequence-model.md)                 | Sequence document hierarchy, alphabets, `SequenceCharSequence`, quality scores & chromatograms, `PairedReads`, `ImmutableSequence`, sequence lists, `SequenceListOnDisk`, `SequenceListSummary`, memory strategies                     |
| [03-annotations-tracks.md](03-annotations-tracks.md)         | `SequenceAnnotation` structure + full type catalog, intervals, qualifiers, tracks + track managers, lazy track loading, gap-coordinate translation                                                                                     |
| [04-alignment-contig-model.md](04-alignment-contig-model.md) | `SequenceAlignmentDocument`, contigs, reference sequences, mates/paired reads, `EndGapsManager`, consensus annotations, combined documents, sequence selection                                                                         |
| [05-storage-query.md](05-storage-query.md)                   | Database services, folder hierarchy, hidden elements, per-document auxiliary XML, the query model (`Query`/`Condition`/`QueryField`), retrieve callbacks, file-backed serialization, lazy loading, storage design implications for Mab |
| [06-plugin-system.md](06-plugin-system.md)                   | `GeneiousPlugin` surface, extension-point inventory, lifecycle & threading rules, `.gplugin` packaging, API versioning, dependencies, testing                                                                                          |
| [07-operations-options.md](07-operations-options.md)         | `DocumentOperation` contract, selection signatures, the `Options` system, `GeneiousActionOptions`, annotation generators, alignment operations, assemblers, inter-plugin invocation                                                    |
| [08-viewers-ui.md](08-viewers-ui.md)                         | Document viewers, sequence graphs, viewer extensions, inter-viewer messaging, printing; which parts are Swing-specific                                                                                                                 |
| [09-io-formats.md](09-io-formats.md)                         | File importer/exporter contracts, format auto-detection, streaming import callbacks                                                                                                                                                    |

## How to use these docs

- **Designing a Mab data structure** (sequences, annotations, alignments,
  documents): read 01–04. They describe the _shape_ of Geneious's data, which
  is the best guide for Mab's Rust types.
- **Designing Mab storage/persistence**: read 05 (plus the serialization
  sections of 01 and the on-disk sequence lists in 02).
- **Designing Mab's operation/analysis pipeline**: read 06–07.
- **Designing Mab's UI/view layer**: read 08; note most of it is
  Java/Swing-specific and only the _concepts_ transfer to Iced.

## Conventions used in these docs

- Type names are given as Geneious names (e.g. `SequenceAnnotation`) with
  package context on first mention per section.
- Coordinates: unless stated otherwise, **sequence indices in the core data
  model are 0-based** (`SequenceCharSequence`, `Interval`); **annotation
  intervals are 1-based inclusive** (`SequenceAnnotationInterval`). This dual
  convention is explicit in the API and worth remembering.
- Each doc ends with a **Mab notes** section: short implications for the Rust
  implementation. These are opinions/starting points, not decisions.

## Big-picture takeaways (structure focus)

1. **Two-layer documents.** Domain content (`PluginDocument`) is always wrapped
   in a platform-owned envelope (`AnnotatedPluginDocument`) carrying: editable
   name/field values, user notes, URN, revision numbers, provenance links,
   per-document auxiliary XML, unread flags, database location. The envelope is
   never implemented by domain code.
2. **Typed attribute schema.** Both documents and search queries are built on
   `DocumentField` — a declared, typed attribute (name, description, code,
   value type, visibility, editability). ~80 standard fields exist covering
   sequences, alignments, contigs, chromatograms, trees.
3. **Everything is XML with a binary escape hatch.** All persistence flows
   through JDOM XML elements; large binary data is externalized via
   `FILE_DATA_ATTRIBUTE_NAME` (temp file reference that the database copies).
4. **Scale handled by lazy layers.** Chromosome sequences, million-read lists,
   alignment columns, and track annotations all have dedicated lazy/on-disk
   structures (`SequenceListOnDisk`, `ImmutableSequence`, `EndGapsManager`,
   `SequenceGapInformation`, lazy `SequenceTrack`s) that are _part of the data
   model_, not an afterthought.
5. **Declarative input.** `Options` is a schema of typed widgets that the
   platform renders, persists, validates, and that other plugins can drive
   programmatically — the single parameter mechanism across all operations.
