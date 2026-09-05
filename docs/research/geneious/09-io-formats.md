# 09 — File Import/Export Contracts

Package: `...publicapi.plugin` (`DocumentFileImporter`,
`DocumentFileExporter`, `DatabaseFolderImporter`),
`...publicapi.utilities` (`ImportUtilities`, `ProgressInputStream`).

---

## DocumentFileImporter

`public abstract class DocumentFileImporter` — parses a file and yields
documents **incrementally**.

### Declaration

| Member                                                                     | Notes                                                                                                                                                                  |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `getPermissibleExtensions()` → `String[]`                                  | e.g. `{".fasta", ".fa"}` (case-insensitive in practice)                                                                                                                |
| `getFileTypeDescription()`                                                 | UI label for the format                                                                                                                                                |
| `tentativeAutoDetect(File, String fileContentsStart)` → `AutoDetectStatus` | Cheap sniffing on the first bytes of the file: `REJECT_FILE` (certainly wrong format), `ACCEPT_FILE` (looks right, success not guaranteed), `MAYBE` (unknown)          |
| `supportsAddingDocumentsInBackgroundThread()`                              | Opt-in parallelism: documents may be processed on a background thread, in which case `ImportCallback.addDocument` **can return null** — don't rely on its return value |

### Import contract

```java
void importDocuments(File file, ImportCallback callback,
    ProgressListener progress) throws IOException, DocumentImportException;
```

- **Streaming, callback-based**: documents are pushed to the callback as they
  are parsed — the platform adds them to the local repository _while the
  file is still being read_. Nothing is returned as a list.
- `ImportCallback.addDocument(PluginDocument)` — one document at a time.
- Progress: wrap the file in `ProgressInputStream(progress, file)` for
  byte-level progress; use `progress.setMessage(...)` for counts; check
  `progress.isCanceled()`.
- Failures: `DocumentImportException(message[, cause])`;
  `DocumentImportException.Canceled` for user cancellation.
- Typical implementation shape (SDK FASTA example): line loop → accumulate
  residues → on each new header, construct
  `new DefaultNucleotideSequence(name, residues)` and `callback.addDocument`.
- Alternative shapes suggested by the javadoc: a custom `PluginDocument`
  whose constructor takes a `File`; or reuse existing document types.

---

## DocumentFileExporter

`public abstract class DocumentFileExporter` — writes documents to one file.

| Member                                                         | Notes                                                                 |
| -------------------------------------------------------------- | --------------------------------------------------------------------- |
| `getFileTypeDescription()`                                     | UI label                                                              |
| `getDefaultExtension()`                                        | e.g. `".fasta"`                                                       |
| `getSelectionSignatures()`                                     | Which documents can be exported (same signature system as operations) |
| `mayDiscardInformation()`                                      | Whether export is lossy (shown to user)                               |
| `export(File, AnnotatedPluginDocument[], progress[, Options])` | Write all selected documents to the single file                       |

- `Options` variant: exporters may prompt for export parameters (the platform
  renders them like operation options).
- Cancellation etiquette (SDK example): check
  `progressListener.isCanceled()` per document and **delete the partial
  file** on cancel.
- Use `CompositeProgressListener` nested per-document/per-sequence for
  fine-grained progress.
- `DocumentFileExporterAndExternalViewer` — export + open in an external app.
- `ExportableDocument` (+ `Format`) — documents that can export themselves
  independent of exporter plugins.

---

## Folder-level import

`DatabaseFolderImporter` (`GeneiousPlugin.getDatabaseFolderImporters()`) —
imports an entire directory, optionally reconstructing a folder/database
hierarchy rather than a flat document list.

---

## Dispatch helpers

- `PluginUtilities.importDocuments(File, progress)` →
  `List<AnnotatedPluginDocument>` — platform picks the matching importer.
- `PluginUtilities.exportDocuments(File, docs...)` — platform picks an
  exporter that accepts the documents and format.
- `ImportUtilities` — additional import helpers for various formats.

Format resolution order (inferred from the API): file extension candidates →
each candidate importer's `tentativeAutoDetect(file, contentsStart)` →
user disambiguation when several return `MAYBE`/`ACCEPT_FILE`.

---

## Mab notes

- The streaming callback importer is exactly the shape to keep in Rust
  (iterator/`impl Stream<Item = Document>` or a sink closure) — importing
  multi-GB files must not build an in-memory list, and documents should be
  persisted as they arrive.
- The two-phase detection (extension filter + cheap content sniff) is simple
  and effective; keep `REJECT/ACCEPT/MAYBE` tri-state rather than booleans.
- Lossy-export declaration (`mayDiscardInformation`) is a good trust
  mechanism; format conversions in genomics are often lossy (GenBank → FASTA).
- Options-driven export parameters generalize cleanly to Rust via the same
  parameter-schema system as operations ([07](07-operations-options.md)).
- Deleting partial output on cancellation is a small correctness detail worth
  codifying in Mab's export pipeline.
