# 01 — Core Document Model

Packages: `...publicapi.documents`, `...publicapi.documents.types`.

Geneious persists _everything_ as documents. There are two layers that must be
understood separately:

```
AnnotatedPluginDocument   (platform-owned envelope; one per stored item)
  ├── identity: URN, name, revision numbers, creation/modified dates
  ├── user-editable state: field values, notes, unread flag, color
  ├── provenance: parent/descendant operation records, document history
  ├── aux storage: "additional XML" keyed by string, optionally per-user
  └── wraps ──▶ PluginDocument   (domain content; plugin-defined type)
```

Domain code implements `PluginDocument`; only the platform creates
`AnnotatedPluginDocument` (via
`DocumentUtilities.createAnnotatedPluginDocument(...)`).

---

## PluginDocument (domain content interface)

`public interface PluginDocument extends XMLSerializable`. Implementations
**must be public classes with a public no-arg constructor** (used before
`fromXML`).

### Contract methods

| Method                         | Returns               | Notes                                                                                        |
| ------------------------------ | --------------------- | -------------------------------------------------------------------------------------------- |
| `getName()`                    | `String`              | Default name; the envelope can override/rename                                               |
| `getDescription()`             | `String`              | One-line description for the document table                                                  |
| `getCreationDate()`            | `Date` (nullable)     | If null, envelope uses instantiation date                                                    |
| `getURN()`                     | `URN` (nullable)      | Unique ID for externally-sourced docs; locally generated docs return null (platform assigns) |
| `getDisplayableFields()`       | `List<DocumentField>` | Declares table columns; should work _before_ `fromXML` completes                             |
| `getFieldValue(String code)`   | `Object`              | Value for a declared field; type must match `DocumentField.getValueType()`                   |
| `toHTML()`                     | `String`              | Text-view rendering; capped at `MAXIMUM_HTML_LENGTH` (JTextPane perf)                        |
| `toXML()` / `fromXML(Element)` | JDOM `Element`        | Full round-trip serialization (see below)                                                    |

### Constants and marker interfaces

| Member                                                | Purpose                                                                                                  |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `FILE_DATA_ATTRIBUTE_NAME` (`xmlFileData`)            | Binary-data escape hatch in XML (see [05-storage-query.md](05-storage-query.md#file-backed-binary-data)) |
| `MAXIMUM_HTML_LENGTH`                                 | Cap for `toHTML()` output                                                                                |
| `MODIFIED_DATE_FIELD`                                 | Standard modified-date field                                                                             |
| `PluginDocument.ReferencedDocumentsAlwaysLoaded`      | Marker: preload referenced docs before handing to viewers                                                |
| `PluginDocument.ReferencedDocumentsNotLoaded`         | Marker: never need referenced docs loaded                                                                |
| `PluginDocument.SizeRequiredToLoadIntoMemoryProvider` | Report estimated memory needed to load (used for memory checks before loading big docs)                  |

### Referenced documents (inter-document links)

If a document stores URNs of other documents in its XML it must serialize them
with `URN.toXML(elementName)` so the platform can detect the references.
Consequences:

- Referenced docs are resolvable via `DocumentUtilities.getDocumentByURN(URN)`.
- On export/share, the platform follows references and bundles/copies them.
- Databases maintain reference counts and "invisible regions" for docs only
  kept alive by references (see [05](05-storage-query.md)).
- The envelope exposes `getReferencedDocuments()` split into
  `getStronglyReferencedDocuments()` and `getWeaklyReferencedDocuments()`
  (`URN.toXMLAsWeakReference` for weak refs).

---

## XMLSerializable (serialization contract)

`public interface XMLSerializable` — the universal persistence interface
(documents, queries, options, annotations, intervals, selections all implement
it).

- `toXML()` → JDOM `Element`; recommended root element name is
  `ROOT_ELEMENT_NAME` (then no `"type"` attribute allowed → compact storage).
- `fromXML(Element)` restores state; the input element may be reused — clone
  before mutating.
- Alternative for non-documents: throw `UnsupportedOperationException` from
  `fromXML` and provide a (possibly private) constructor taking a single
  `Element` — enables final fields.
- Serialization goes through `XMLSerializer.classToXML(String, XMLSerializable)`
  / `classFromXML(Element[, Class])` which records the class for
  deserialization.
- `XMLSerializableWithProgress` variant: `fromXML(Element, ProgressListener)`
  for huge documents.

### Version compatibility

- `XMLSerializable.OldVersionCompatible` + `VersionSupportType`:
  implementations declare the oldest Geneious major version they can serialize
  _to_ (`getVersionSupport(...)`), enabling forward/backward file
  compatibility. Docs are strongly encouraged to implement this, otherwise
  they are only compatible with the latest version.
- Serialization APIs frequently take a `Geneious.MajorVersion` parameter
  (e.g. `SequenceCharSequence.toXML(majorVersion, progressListener)`).

---

## AnnotatedPluginDocument (the envelope)

`public abstract class AnnotatedPluginDocument implements XMLSerializable`.
Plugins **never** subclass it.

### Attribute inventory

| Attribute                      | Accessors                                                                                                                                      | Notes                                                                                                |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Wrapped document               | `getDocument()`, `getDocumentOrThrow(Class<E>)`, `getDocumentOrCrash()`, `getDocumentOrNull()`, `getDocumentOrThrow(warn, progress, Class<E>)` | Loading may fail (memory, network) — the `OrThrow` variants choose the exception type                |
| Document class without loading | `getDocumentClass()`                                                                                                                           | Type checks without paying load cost                                                                 |
| Name                           | `getName()` / `setName(String)`                                                                                                                | Overrides wrapped doc's default name                                                                 |
| Field values                   | `getFieldValue(DocumentField\|String)`, `setFieldValue(...)` (visible), `setHiddenFieldValue(...)`                                             | Cached from wrapped doc so the table renders without loading content                                 |
| Displayable fields             | `getDisplayableFields()`, `getExtendedDisplayableFields()`                                                                                     | Extended = base + name/description/created + all note fields (`noteCode.fieldCode`)                  |
| Notes                          | `getDocumentNotes(boolean isForEditing)` → `DocumentNotes`                                                                                     | Changes require explicit `saveNotes()`                                                               |
| URN                            | `getURN()`                                                                                                                                     | Primary database key                                                                                 |
| Revision numbers               | `getRevisionNumber()` (envelope), `getInternalPluginDocumentRevisionNumber()`; also `UrnWithRevisionNumber` pairs                              | Optimistic concurrency / change detection                                                            |
| Database location              | `getDatabase()` / `setDatabase(DatabaseService)`                                                                                               | Every stored doc knows its folder                                                                    |
| Source service                 | `setSourceService(...)`                                                                                                                        | Where to download full doc if this is a summary                                                      |
| Creation date                  | `getCreationDate()`                                                                                                                            |                                                                                                      |
| Size                           | `getSize()`                                                                                                                                    | Uncompressed bytes                                                                                   |
| Unread flag                    | `isUnread()` / `setUnread(boolean)`                                                                                                            | "retrieved by agent, not yet seen by user"                                                           |
| Deletion state                 | `isDeletedFromWritableDatabaseService()`                                                                                                       |                                                                                                      |
| Provenance                     | `getParentOperationRecord()`, `getDescendantOperationRecords()`, `add/removeDescendantOperationRecord(URN)`                                    | Links to `OperationRecordDocument`s                                                                  |
| History                        | `getDocumentHistory()` → `DocumentHistory`                                                                                                     |                                                                                                      |
| Additional XML                 | `getAdditionalXml(key, isPerUser)`, `setAdditionalXml(key, isPerUser, Element)`, batch map variants with `ElementProvider`                     | Arbitrary keyed XML side-channel stored with the document; `isPerUser` keeps it out of shared copies |
| Temporary fields               | `get/setTemporaryFieldValue(String, Object)`                                                                                                   | In-memory only; e.g. `TEMPORARY_FIELD_KEY_..._LOCATION` for file paths                               |
| Raw XML access                 | `getPluginDocumentXml(progress)`, `toXMLExcludingInternalPluginDocument([majorVersion])`                                                       | Basis for database storage implementation                                                            |
| Lock                           | `getLock()`                                                                                                                                    | Object used while mutating in-memory contents                                                        |

### Lifecycle / persistence behavior

- `save()` — save only envelope changes (name, fields, notes).
- `saveDocument()` — save wrapped `PluginDocument` **and** envelope.
- `saveDocument(PluginDocument)` — replace content atomically and save.
- `batchSave(Collection, updateModifiedDate, progress)` — bulk summary save.
- `clearInternalDocumentCache()` — drop cached loaded content (memory
  management).
- `toSummaryDocument()` — produce a lightweight clone whose content is a
  summary placeholder (`SummaryDocument`), keeping metadata; full content can
  be re-fetched from `getSourceService()`.
- `changeReferencedDocumentURNs(Map<URN,URN>)` — rewrite references (used when
  copying docs between databases).
- `documentChanged(Element)` — replace contents from new XML (remote update).
- `addWeakReferenceDocumentListener(DocumentListener)` — change notifications.
- `cachePluginDocumentXmlAndAdditionalXmlLocallyForDocumentsInAnyDatabase(...)`
  — pre-download network-database content before offline-ish use.

---

## URN (universal resource name)

`public class URN` — immutable, `namespace:assigner:element` after an
`identifier:` prefix.

- Format: `identifier:namespace:assigner:element`,
  e.g. `urn:sequence:ncbi:genbank/AF00001`.
- Public fields: `namespace` (rules org), `assigner` (naming org), `element`
  (the item).
- `generateUniqueLocalURN()` — local docs, assigner `"."`.
- `generateExcludedDuringImportUrn()` — marker URNs.
- `isLocal()` / `isCloud()` (deprecated: cloud URNs now report local since
  2022.2).
- `toXML(elementName)` vs `toXMLAsWeakReference(elementName)` — strong vs weak
  reference serialization.
- Copy semantics: copying a `URN` lets the in-memory document associated with
  the original become garbage-collectable; both copies still resolve via
  `DocumentUtilities.getDocumentByURN`. So URNs double as soft-reference
  handles to loaded documents.
- `UrnWithRevisionNumber` — URN + revision, for stale-write detection.

---

## DocumentField (typed attribute schema)

`public final class DocumentField implements XMLSerializable` — describes one
data item of a document **or** one searchable query field (same object serves
both; query fields are built from document fields).

### Structure

- Human-readable name, description, **code** (stable machine key), value type
  (`Class`), visibility (`visibleInTable`-style booleans), editability;
  extended ctor also takes "user-changable" and a since-version string
  (e.g. `new DocumentField("Adenine Frequency","","adenineFrequency",
  Integer.class, true, false, true, "2021.0.0")`).
- Static factories: `createStringField`, `createIntegerField`,
  `createDateField`, ... (prefer over the raw constructor).
- `DocumentField.SequenceType` enum: special sequence markings (vectors,
  references, etc.).

### Assigning values

- Plugin documents: declare in `getDisplayableFields()` + `getFieldValue`.
- Envelope-level: `AnnotatedPluginDocument.setFieldValue/setHiddenFieldValue`.
- Generators: via `SequenceAnnotationGenerator.AnnotationGeneratorResult`.
- Search results: via the `searchResultProperties` map passed to
  `RetrieveCallback.add(doc, properties)`.

### Standard field catalog (`DocumentField` statics, ~80 fields)

Identity & metadata: `NAME_FIELD`, `DESCRIPTION_FIELD`, `CREATED_FIELD`,
`MODIFIED_DATE_FIELD`, `URN_FIELD`, `NOTES`, `COLOR`, `UNREAD_FIELD`,
`ACCESSION_FIELD`, `DATABASE_LOCATION_FIELD`,
`DELETED_DOCUMENT_ORIGINAL_LOCATION_FIELD`.

Organism/taxonomy: `ORGANISM_FIELD`, `COMMON_NAME_FIELD`, `TAXONOMY_FIELD`,
`MOLECULE_TYPE_FIELD`, `TOPOLOGY_FIELD` (linear/circular), `GENETIC_CODE_FIELD`,
`SAMPLE_NAME_FIELD`, `DATA_TYPE_FIELD` (sequencing platform),
`PAIRED_DATA_TYPE_FIELD`.

Sequence metrics: `SEQUENCE_LENGTH`, `SEQUENCE_TYPE`, `SEQUENCE_COUNT`,
`NUCLEOTIDE_SEQUENCE_COUNT`, `PROTEIN_SEQUENCE_COUNT`,
`NUCLEOTIDES_COUNT`, `FIRST_SEQUENCE_RESIDUES`, `GC_PERCENT`,
`NUMBER_OF_STOP_CODONS`, `MAXIMUM_SEQUENCE_LENGTH`, `MINIMUM_SEQUENCE_LENGTH`,
`NUCLEOTIDE_SEQUENCES_WITH_MATES_COUNT`,
`NUCLEOTIDE_SEQUENCES_WITH_QUALITY_COUNT`, `TRIMMED_SEQUENCES_COUNT`,
`SEQUENCE_LIST_ORDERING_REVISION_NUMBER`.

Chromatogram/quality: `AMBIGUITIES`, `HIGH_QUALITY_PERCENT`,
`MEDIMUM_QUALITY_PERCENT`, `LOW_QUALITY_PERCENT`, `POST_TRIM_LENGTH`, `BIN`,
`BIN_REASON`, `BINNING_FRAME`, `BINNING_GENETIC_CODE`.

Alignment: `ALIGNMENT_METHOD_FIELD`, `ALIGNMENT_OPTIONS_FIELD`,
`ALIGNMENT_SCORE_FIELD`, `ALIGNMENT_PERCENTAGE_IDENTICAL`,
`ALIGNMENT_SIMILARITY`, `ALIGNMENT_MATCH_REGIONS_FIELD`, `DISAGREEMENTS`,
`INDEL_DISAGREEMENTS`, `IS_FREE_END_GAPS`.

Contig/assembly: `CONTIG_MEAN_COVERAGE`,
`CONTIG_REFERENCE_SEQUENCE_INDEX`, `CONTIG_REFERENCE_SEQUENCE_LENGTH`,
`CONTIG_PERCENTAGE_OF_REFERENCE_SEQUENCE_COVERED`, `CONSENSUS_SEQUENCE_LENGTH`,
`CONSENSUS_SOURCE_SEQUENCES_FIELD`, `REFERENCE_SEQUENCE_NAME`,
`NUMBER_OF_DIFFERENT_MAPPED_READS`, `NUMBER_OF_MAPPED_LOCATIONS`,
`MAPPING_QUALITY`, `HAS_ANY_READS_MAPPED_TO_MULTIPLE_LOCATIONS`.

Search hits: `E_VALUE_FIELD`, `BIT_SCORE`.

Search-only helpers: `ALL`, `DOCUMENT_SEARCHING_ANNOTATIONS_FIELD`,
`DOCUMENT_SEARCHING_ANNOTATION_NAME_FIELD`,
`DOCUMENT_SEARCHING_ANNOTATION_TYPE_FIELD`, `DOCUMENT_SEARCHING_TRACKS_FIELD`,
`ALL_STANDARD_FIELDS` (aggregate list).

---

## Notes system (structured user metadata)

- `DocumentNoteType` — schema: code, name, description, list of
  `DocumentNoteField`s, visibility flags (`isVisible`,
  `isDefaultVisibleInTable`), `isStoredInConnectedNonLocalDatbase`,
  modification date. Users can create/edit note types.
- `DocumentNote` — one instance of a type on a document, holding field values.
  **Max one instance per note type per document.**
- `DocumentNoteField` (+ `DocumentNoteField.Types`) — typed fields within a
  note.
- Access via `DocumentNoteUtilities`; mutation via
  `AnnotatedPluginDocument.getDocumentNotes(isForEditing)` followed by
  `DocumentNotes.saveNotes()`.
- Note fields surface as document-table columns with codes
  `noteCode.fieldCode` (part of `getExtendedDisplayableFields()`).

---

## Provenance — OperationRecordDocument

`public class OperationRecordDocument implements PluginDocument` — one record
per operation run, linking input → output documents.

- Created automatically (with _inactive_ links) for every `DocumentOperation`
  run; stored wrapped in envelopes in a **hidden area** of the user's
  database.
- Constructor data: input document URNs, optional additional parent URNs,
  `operationId`, `additionalInformation` string, timestamp, and optional
  `optionsForRecreatingDescendants` XML → makes the link **active**.
- Active links: descendants can be regenerated when parents change
  (`DocumentOperation.getOptions(Element, parents, inputs)` recreates the
  options). Non-operations can create links by constructing a record and
  calling `linkDocumentsInDatabase(progress)`.
- Region mapping: `addIntervals(targetUrn, sourceUrn, intervals)` records
  which sequence region in a parent corresponds to a region in a descendant.
- `FAKE_OPERATION_ID_PREFIX` — display-only pseudo operations.
- Envelope access: `getParentOperationRecord()` /
  `getDescendantOperationRecords()`; resolve via
  `DocumentUtilities.getDocumentByURN`.

Related: `DocumentHistory` / `DocumentHistoryEntry` /
`DocumentHistoryEntryField` — per-document edit history.

---

## Other document-layer types

| Type                                   | Role                                                                                   |
| -------------------------------------- | -------------------------------------------------------------------------------------- |
| `SummaryDocument`                      | Marker for lightweight placeholder content (summary only, full doc downloadable)       |
| `AliasDocument`                        | Pointer/shortcut document to another document                                          |
| `DocumentCollection`                   | Groups documents                                                                       |
| `ExportableDocument` (+ `Format`)      | Documents that can export themselves in defined formats                                |
| `FolderViewDocument`                   | Hidden doc describing a folder's search results (see [05](05-storage-query.md))        |
| `Renamable`                            | Marker for rename support                                                              |
| `AdditionalSearchContent` (+ `Result`) | Extra text a document contributes to full-text search (see TextDocument example below) |
| `DocumentSearchCache`                  | Caching for document-search dialogs                                                    |

### `documents.types` package

- `TreeDocument` (base for tree-holding docs) → `SameTaxaTreesDocument`
  (multiple trees, same taxa) → `RootedTreeDocument`; `PhylogenyDocument`
  extends `DefaultAlignmentDocument` and adds trees whose tips are the
  alignment sequences.
- `PublicationDocument` — `getTitle()`, `getAbstract()`, `getAuthors()`
  (`Author` type), `getPublicationDate()`; `JournalArticleDocument` refines
  it; both have `Utils` helpers.
- `MolecularStructureDocument` — 3D structures (implementations in
  `implementations.structure`: PDB, CML, Mol, XYZ, Hin, Gpr, Nwo formats).
- `TaxonomyDocument` with nested `Taxon` and `TaxonomicLevel`.

---

## Custom document type example (TextFiles plugin)

The SDK's `TextFiles` example shows the complete contract for a novel
document type:

```java
public class TextDocument implements PluginDocument, AdditionalSearchContent {
    private String text;          // content
    private String name;          // document name
    private Date creationDate;    // creation date

    public TextDocument() {}      // required no-arg ctor for fromXML

    public Element toXML() {      // manual XML mapping
        Element root = new Element("TextDocument");
        root.addContent(new Element("name").setText(name));
        root.addContent(new Element("date").setText("" + creationDate.getTime()));
        root.addContent(new Element("text").setContent(new CDATA(text)));
        return root;
    }
    public void fromXML(Element doc) { /* inverse */ }

    // Table columns:
    public List<DocumentField> getDisplayableFields() { ... word count field ... }
    public Object getFieldValue(String code) { ... }

    // Full-text search content (optionally per-field):
    public Result[] getSearchContent() {
        return new Result[]{ new Result(null, text),
                             new Result(everySecondWordField, getEverySecondWord()) };
    }
    public URN getURN() { return null; }  // locally generated docs: null
}
```

Registration with the platform (icons/type metadata):

```java
public DocumentType[] getDocumentTypes() {
    return new DocumentType[]{ new DocumentType("Text Files", TextDocument.class, null) };
}
```

`DocumentType<T>` binds: human-readable name, document class, optional
override class, icon. The platform ships constants for the built-in types
(`NUCLEOTIDE_SEQUENCE_TYPE`, `ALIGNMENT_TYPE`, `CONTIG_TYPE`, `TREE_TYPE`,
`PUBLICATION_TYPE`, ...). A document can override its type at runtime by
returning a class from `getFieldValue(OVERRIDE_DOCUMENT_TYPE_KEY)`.

---

## Mab notes

- The envelope/content split maps well to Rust: a generic
  `DocumentHandle<Content>` or a metadata table keyed by document id, rather
  than inheritance. Keep content immutable behind the handle (Geneious leans
  heavily on immutability + copy-on-write, e.g. `SequenceCharSequence`).
- `DocumentField` ≈ a declared, typed column schema. Mab should decide early
  whether attributes are open-ended (key/value) or schema-declared; Geneious
  chose schema-declared + a hidden-field escape hatch + notes for user-defined
  schemas.
- The "additional XML" side-channel is a pragmatic lesson: sometimes
  per-document auxiliary data (viewer state, per-user prefs) is best stored
  beside the document rather than inside its content type.
- URN behavior (identity + soft handle + revision) suggests a Rust design
  with a document-id type that can optionally pin revision numbers for
  stale-write detection.
- Provenance records are cheap because they are just documents referencing
  URNs; an explicit provenance graph in Mab's storage layer would generalize
  this.
