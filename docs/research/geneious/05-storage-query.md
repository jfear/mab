# 05 — Storage, Databases & Query Model

Packages: `...publicapi.databaseservice`, plus serialization hooks in
`...publicapi.documents`.

This is the layer most worth mining for Mab storage design: how Geneious
stores documents, folders, auxiliary data, and how querying is expressed.

---

## Service hierarchy

```
GeneiousService                      (left-panel entry; abstract)
└── DatabaseService                  (answers queries with documents)
    └── WritableDatabaseService      (adds/deletes docs, folder tree, hidden data)
        ├── Local database  (unique id "LocalDocuments")
        ├── Shared (server) databases
        ├── Cloud database root ("Cloud")
        └── 3rd-party services (e.g. ExampleGeneiousService over a FASTA file)
    PartiallyWritableDatabaseService (read-mostly variant)
```

Services are globally addressable by string id:
`PluginUtilities.getGeneiousService("LocalDocuments")`,
`PluginUtilities.getGeneiousServices()`. Well-known ids are constants on
`PluginUtilities`: `LOCAL_DATABASE_SERVICE_UNIQUE_ID`,
`CLOUD_DATABASE_SERVICE_UNIQUE_ID`, `FAVORITE_SERVICE_UNIQUE_ID`,
`SEARCH_RESULTS_SERVICE_UNIQUE_ID`, `ALIAS_UNIQUE_ID_PREFIX`.

### GeneiousService basics

`getUniqueID()`, `getName()`, `getDescription()`, `getHelp()`, `getIcons()`,
`initializeService(GeneiousServiceListener)` (called once; may be on a
non-AWT thread), actions (`getActionsAlwaysEnabled()`,
`getActionsEnabledWhenServiceSelected()`), overlay icons.

### DatabaseService surface

| Member                                                                                                                                                                                                                                           | Notes                                                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------- |
| `getSearchFields()` → `QueryField[]`                                                                                                                                                                                                             | Fields users may search on                                                                  |
| `retrieve(Query, RetrieveCallback, URN[] urnsToNotRetrieve)`                                                                                                                                                                                     | **Core implementation point**: stream results into the callback, skipping URNs already held |
| `retrieve(Query, ProgressListener)` / `retrieve(String)`                                                                                                                                                                                         | Convenience list-returning wrappers                                                         |
| `retrieve(URN[], RetrieveCallback)`                                                                                                                                                                                                              | Fetch by id                                                                                 |
| `retrieve(summaryDocuments, ...)`                                                                                                                                                                                                                | Upgrade summary docs to full docs                                                           |
| `isBrowsable()`                                                                                                                                                                                                                                  | Documents shown when the service is selected                                                |
| `getExtendedSearchOptions(isAdvanced)`                                                                                                                                                                                                           | Extra search-panel options                                                                  |
| `showAdvancedSearchByDefault()`                                                                                                                                                                                                                  |                                                                                             |
| Sequence search: `getSequenceSearchPrograms(queryType)` (e.g. BLAST programs), `getSequenceSearchOptions(program)`, `sequenceSearch(querySeq, program, options, callback)`, `batchSequenceSearch(...)`, `getDocumentTypesForSequenceSearch(...)` | Similarity-search integration                                                               |
| `isDocumentUnreadStatusEnabled()`                                                                                                                                                                                                                | Per-service unread flags                                                                    |
| `getOpenInWebUrlForDocuments(docs)`                                                                                                                                                                                                              |                                                                                             |
| `locateSummaryDocument(actual, summaries)`                                                                                                                                                                                                       | Match a downloaded doc to its summary                                                       |
| Listeners: `addDatabaseServiceListener(...)` / `addWeakDatabaseServiceListener(...)`                                                                                                                                                             | See events below                                                                            |

`DatabaseService.SequenceSearchQueryType` distinguishes query kinds for
sequence search.

---

## WritableDatabaseService (folder tree + persistence)

A `WritableDatabaseService` **is** a folder; child folders are child service
instances. Local and shared databases implement the same interface.

### Document lifecycle

- Add: `addDocumentCopy(doc, progress)` / `addDocumentCopies(list|DocumentsToAdd,
  progress)` / `addDocumentCopyWithProperties(doc, progress, properties[,
  urn])`. Documents are always added **as copies**; the added copy gets a
  new URN (or a pre-reserved one from `createUrnForDocumentSoonToBeAdded()`).
- `WritableDatabaseService.DocumentsToAdd` / `DocumentToAdd` — batch add with
  per-document target sub-folders and properties.
- Save: `saveDocumentsAnywhereWithinDatabase(docs, onlyAnnotatedChanged,
  progress)` (+ `batchSave` on the envelope side).
- Remove: `removeDocument(s)`; soft-delete behavior via
  `DeletedItemsType` enum: `None`, `Standard` (platform-managed trash),
  `CustomHandling`.
- Move: `moveDocument(s)`, `canMoveTo(destination)`;
  `documentMoved` listener event.

### Folder structure

- `createChildFolder(name)` / `createChildFolders(names, progress)`;
  folder depth limits enforced (`checkFolderDepth`, `checkChildDepths`).
- `canRemoveChildFolder(name)`, `renameFolder(name)`, `canBeRenamed()`,
  `allowChangingFolderProperties()`.
- `ColorableService` — folder colors (`canChangeFolderColor()`).
- Favorites: `addIsFavoritedChangedListener(...)`.
- Browse contents via `retrieve(Query.Factory.createBrowseQuery(), ...)`.

### Hidden areas & arbitrary XML

- Hidden per-user or shared folders: `getHiddenFolder(name, isPerUser)`.
- **Hidden elements**: arbitrary named XML blobs stored in the database —
  `addHiddenElement(name, Element, progress)` (or auto-named),
  `addHiddenElements(...)`, retrieval/removal by name. Used for things like
  operation records and other internal state.
- Hidden document regions: referenced-only documents and deleted-but-
  referenced documents live in invisible regions (see contracts below).

### Folder views

`createFolderViewFolder(name, folderViewDocument)` — a folder whose contents
are best represented by a `FolderViewDocument` (a `PluginDocument` exposing
`FolderView`): metadata about a search result set enabling a query-centric
view (e.g. BLAST results folder).

### Implementation contracts (for writing a WritableDatabaseService)

1. Storage format: persist each document as
   `AnnotatedPluginDocument.toXMLExcludingInternalPluginDocument()` +
   `document.getDocumentOrNull().toXML()`; reconstruct with
   `DocumentUtilities.createAnnotatedPluginDocument(Element, ElementProvider,
   URN)` — the `ElementProvider` lets the content load lazily.
2. Every returned document must have `setDatabase(service)` set before
   external code sees it.
3. **Reference counting**: on add, increment counts of referenced docs
   already present; copy not-present references into an invisible region. On
   remove, referenced docs move to the invisible region instead of deletion
   until unreferenced.
4. Network databases: keep one service instance per remote folder across
   login sessions; in-memory envelopes survive logout until GC.
5. Pre-caching helpers:
   `cachePluginDocumentXmlAndAdditionalXmlLocally(docs, progress)` (no-op for
   local; downloads for network).

---

## Query model

Queries form a small AST, all implementing `Query` (which is itself
`XMLSerializable`):

```
Query
├── BasicSearchQuery            getSearchText()            (free text / browse)
├── AdvancedSearchQueryTerm     getField() + getCondition() + getValues()
└── CompoundSearchQuery         getOperator() ∈ {AND, OR} + getChildren()
```

- `Query.isBrowse()` — folder listing query (no filter).
- `Query.getExtendedOptionValue(code)` — service-specific option values.

### Query.Factory

| Factory method                                                            | Produces                                         |
| ------------------------------------------------------------------------- | ------------------------------------------------ |
| `createBrowseQuery()`                                                     | All docs in the folder (excluding child folders) |
| `createQuery(String)`                                                     | Full-text search across the database             |
| `createFieldQuery(DocumentField, Condition, value[s][, extendedOptions])` | Field-condition term                             |
| `createAndQuery(Query[], opts)` / `createOrQuery(Query[], opts)`          | Compounds                                        |
| `createExtendedQuery(text, opts)`                                         | Text + options (e.g. search subfolders)          |
| `createAliasQuery(searchSubfolders)`                                      | Alias documents                                  |

### QueryField & Condition

`QueryField = DocumentField + Condition[]` — declared per service via
`getSearchFields()`; drives the advanced-search UI and constrains what users
can express.

`Condition` enum (`documents` package):
`EQUAL`, `NOT_EQUAL`, `APPROXIMATELY_EQUAL`, `CONTAINS`, `NOT_CONTAINS`,
`BEGINS_WITH`, `ENDS_WITH`, `GREATER_THAN`, `GREATER_THAN_OR_EQUAL_TO`,
`LESS_THAN`, `LESS_THAN_OR_EQUAL_TO`, `IN_RANGE`,
`STRING_LENGTH_GREATER_THAN`, `STRING_LENGTH_LESS_THAN`, `DATE_AFTER`,
`DATE_BEFORE`, `DATE_AFTER_OR_ON`, `DATE_BEFORE_OR_ON`.

### Extended search options

Services expose extra search knobs as `ExtendedSearchOption` subclasses:
`CheckboxSearchOption`, `TextFieldSearchOption`, `ComboboxSearchOption`,
`DependentComboboxSearchOption`, `TextOrSimilaritySearchOption`.
Standard codes on `WritableDatabaseService`: `KEY_SEARCH_SUBFOLDERS`,
`KEY_SEARCH_TYPE` (text vs similarity), `KEY_WHOLE_WORDS_ONLY`,
`KEY_PROCESS_SPECIAL_QUERY_SYMBOLS`,
`KEY_SEARCH_INDEX_CORRUPT_OR_MISSING_FAIL_WITH_EXCEPTION`
(→ `SearchIndexCorruptOrMissingException`).

### RetrieveCallback (streaming results)

`public abstract class RetrieveCallback extends ProgressListener` — results
stream asynchronously; implement `_add(PluginDocument|AnnotatedPluginDocument,
Map<String,Object> searchResultProperties)`:

- The properties map attaches per-result metadata (e-values, scores) which
  surface as fields without loading content; a
  `SearchResultPropertiesAdjuster` can rewrite properties at the service
  level.
- Progress extras: `setStatus(message, totalDocs, approximated,
  predictedTime, bytesDownloaded, totalBytes)`, `setImage`,
  `setIndeterminateProgress`.
- `acceptsChangesAfterRetrieveCompletes(listener)` — services may keep
  mutating results post-retrieve (e.g. similarity hits downloading).
- Wrappers: `CompositeRetrieveCallback`, `RetrieveCallback.EMPTY`.
- Exceptions: `DatabaseServiceException(message|cause, isRetryable)`,
  `DatabaseServiceException.Canceled`.

### Recipes (from the API FAQ)

```java
// siblings in the same folder:
doc.getDatabase().retrieve(Query.Factory.createBrowseQuery(), ProgressListener.EMPTY);

// everything in the local database:
WritableDatabaseService db = (WritableDatabaseService)
    PluginUtilities.getGeneiousService("LocalDocuments");
db.retrieve(Query.Factory.createQuery(""), ProgressListener.EMPTY);      // all folders
db.retrieve(Query.Factory.createBrowseQuery(), ProgressListener.EMPTY); // root only
```

---

## Serialization storage mechanics

### Envelope/content split on disk

A stored document = envelope XML (metadata, field values, notes, additional
XML, references) **+** content XML (the `PluginDocument`). The envelope can
be read without the content — that is how the document table renders fast and
`getDocumentClass()`/`getFieldValue(...)` work without loading.

### Lazy content loading

`DocumentUtilities.createAnnotatedPluginDocument(Element envelope,
ElementProvider contentProvider, URN)` — the content provider supplies the
`PluginDocument` XML only on demand (`getDocument()`), enabling:

- Memory pressure checks (`SizeRequiredToLoadIntoMemoryProvider`) before load.
- Network downloads with progress (`XMLSerializableWithProgress`).
- Revision tracking (`getInternalPluginDocumentRevisionNumber()`,
  `UrnWithRevisionNumber`) to detect stale copies.
- Cache management (`clearInternalDocumentCache()`).

### File-backed binary data

`PluginDocument.FILE_DATA_ATTRIBUTE_NAME` (attribute name `xmlFileData`):

1. During `toXML()` a document writes large data to a temp file and emits an
   element carrying `xmlFileData="path/to/temp/file"` (may appear on multiple
   elements → multiple files per document).
2. The `WritableDatabaseService` stores a **copy** (or hard link) of that file
   and rewrites the attribute to its own copy's path.
3. On load the document receives the database's file path; it must treat that
   file as **immutable** — changes go to new files.
4. Use `FileUtilities.createTempFile` (auto-deletes when unreferenced) and
   prefer `DocumentUtilities.pluginDocumentFileDataToXml/FromXml`, which add
   corruption detection/handling (needed on Windows network drives).

This is the sanctioned way to keep multi-MB/GB payloads (sequences, quality
data, structures) out of XML.

### Per-document auxiliary data

- **Additional XML** (envelope level): `setAdditionalXml(key, isPerUser,
  Element)` — keyed side-channel, optionally per-user (not shared); batch
  lazy variant with `ElementProvider`. Viewer state, layout settings, etc.
- **Hidden elements** (database level): named XML blobs not tied to a
  document.
- **Temporary fields** (memory only): `setTemporaryFieldValue(...)`.

---

## Events / change propagation

| Mechanism                                                   | Events                                                                                                                                                                                                                                    |
| ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DocumentListener` (via `addWeakReferenceDocumentListener`) | Document content/metadata changed                                                                                                                                                                                                         |
| `DatabaseServiceListener`                                   | `browsableContentsChanged`, `documentCopyAboutToBeAdded(doc)`, `documentMoved(doc, source)`, `hiddenElementsChanged`, `fieldsChanged`, `searchableStatusChanged(isSearchable, message)`, `actionsChanged`, `extendedSearchOptionsChanged` |
| `PluginUtilities.WritableDatabaseServicesListener`          | Root databases added/removed                                                                                                                                                                                                              |
| `GeneiousServiceListener`                                   | Service-level events during `initializeService`                                                                                                                                                                                           |
| `SimpleListener` (org.virion.jam.util)                      | Generic change callback used widely (options changes, favorites, plugin changes)                                                                                                                                                          |

---

## Storage design implications for Mab

Geneious's storage choices, abstracted away from Java:

1. **Envelope vs content, always.** Metadata that must be cheap to list
   (name, fields, notes, provenance, references) is stored separately from
   potentially huge content, with lazy content loading and explicit memory
   estimates before loading. Mab should make this a hard architectural line.
2. **XML everywhere + file escape hatch.** Uniform, versioned, human-readable
   envelopes; blobs externalized as copied files with copy-on-write
   semantics. Rust equivalent: versioned serde envelopes (or SQLite rows) +
   content-addressed blob store; keep the "database copies and rewrites
   paths" ownership rule.
3. **Reference counting inside the store.** Inter-document links are URNs;
   the store keeps referenced-but-invisible docs alive and garbage-collects
   them when unreferenced. Any provenance graph in Mab needs this GC story.
4. **Folders are services.** The folder tree _is_ the database abstraction;
   local and remote are the same trait. A Rust `trait Folder/Database` with
   local/remote impls generalizes this.
5. **Queries are typed ASTs over declared fields**, not string DSLs —
   services declare searchable fields + allowed conditions, and the UI is
   generated from that declaration. Worth copying directly.
6. **Side-channels**: per-user data (`isPerUser`), hidden elements, temp
   fields — separate lifetimes and visibility scopes for auxiliary data.
7. **Indexes are persisted artifacts**: sequence-list caches, end-gaps
   indexes, alignment layouts are all serialized/deserialized independently
   of their source data and can be rebuilt. Treat indexes as disposable,
   versioned cache files.
8. **Unread flags, revision numbers, summary documents**: collaboration and
   network ergonomics live in the storage layer, not the UI.
