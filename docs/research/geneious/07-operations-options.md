# 07 — Operations, Options & Analysis Contracts

Package: `...publicapi.plugin`, plus `jebl.util.ProgressListener` (from the
JEBL library Geneious depends on).

This document covers the _analysis pipeline_ contracts: how work is declared,
parameterized, invoked, and how results flow back. UI placement details are in
[08](08-viewers-ui.md).

---

## ProgressListener (universal cancellation + progress)

`jebl.util.ProgressListener` is threaded through every long-running API:

- `setProgress(fractionCompleted)` → returns `false` if canceled (idiom:
  `if (progress.setProgress(done, total)) throw new
  DocumentOperationException.Canceled();`).
- `isCanceled()`, `setMessage(String)`.
- `ProgressListener.EMPTY` — no-op singleton.
- `CompositeProgressListener(parent, n subtasks | double... weights)` —
  splits a listener into weighted subtasks; `beginSubtask([name])` advances.
  Used everywhere for multi-phase operations.

---

## DocumentOperation (the general analysis contract)

`public abstract class DocumentOperation` — creates new documents from
existing documents (or from nothing). The platform: shows the options panel,
invokes the operation on a background thread, saves results to the selected
repository folder, and records provenance.

### Declaration surface

| Member                                                                          | Purpose                                                                          |
| ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `getActionOptions()` → `GeneiousActionOptions`                                  | Name/description/icon/menu placement (below)                                     |
| `getHelp()`                                                                     | HTML/plain help                                                                  |
| `getSelectionSignatures()` → `DocumentSelectionSignature[]`                     | Accepted inputs (below); **empty array = no inputs** (still a valid menu action) |
| `getUniqueId()`                                                                 | Registry key for `PluginUtilities.getDocumentOperation(id)`                      |
| `getOptions(AnnotatedPluginDocument...)`                                        | Parameter schema (below)                                                         |
| `getOptions(DocumentOperationInput)`                                            | Richer: includes run location + current sequence selection                       |
| `getOptions(SequenceSelection, docs...)`                                        | Adds viewer selection                                                            |
| `getOptions(Element optionsXml, inputs, parents)`                               | Recreate options for re-running an earlier operation (active links)              |
| `getGeneralOptions()`                                                           | Options valid for any input set                                                  |
| `isDocumentGenerator()`                                                         | Whether it produces documents at all                                             |
| `getOrderDependentOperationMessage()`                                           | Warning text when input order matters                                            |
| `loadDocumentsBeforeShowingOptions()` / `loadDocumentsBeforeRunningOperation()` | Force content loading of inputs (+ referenced docs) beforehand                   |
| `cacheDocumentsBeforeShowingOptions()`                                          | Pre-cache network content for responsiveness                                     |
| `canRunOnGeneiousServer()` / `canRunOnLocalGeneious()`                          | Execution-location capability                                                    |
| `getFractionOfTimeToSaveResults()`                                              | Progress hint for the save phase                                                 |

### `performOperation` contract

```java
List<AnnotatedPluginDocument> performOperation(
    AnnotatedPluginDocument[] documents,
    ProgressListener progressListener,
    Options options) throws DocumentOperationException;
```

Rules from the javadoc:

- Runs on a **non-AWT background thread**; any UI interaction beyond the
  options panel must go through `ThreadUtilities.invokeAndWait`.
- **May be invoked concurrently** — implementations must not keep state in
  instance fields.
- Returns the new documents (wrap `PluginDocument`s with
  `DocumentUtilities.createAnnotatedPluginDocuments(...)`), or **null** on
  failure/cancel.
- Cancellation: honor `progressListener.isCanceled()` / `setProgress` return
  value; throw `DocumentOperationException.Canceled`.
- Inputs may record their URNs in output documents (provenance).
- Variants pass `SequenceSelection`; a callback-based variant exists but is
  deprecated ("not fully supported anymore").
- `DocumentOperation.Wrapper` — delegating decorator (everything except
  `getUniqueId()`).
- `DocumentOperation.OperationCallback` — mostly deprecated; still the way to
  set `setOptionsForActivelyLinkedDescendants(...)` for active links.

Related: `DocumentAction` — same shape but explicitly cannot create
documents (pure side effects/UI).

### `DocumentOperationInput`

Richer input bundle: selected documents, current `SequenceSelection`, and
`OperationLocationOptions` (where to run: local vs server — itself an
`Options` instance).

---

## DocumentSelectionSignature (input typing)

`public final class DocumentSelectionSignature implements XMLSerializable` —
declares which document selections enable an operation/viewer/exporter; the
platform enables/disables UI and only invokes with valid selections.

- Built from **atoms**: `DocumentSelectionSignatureAtom` — a
  (document class, min count, max count) triple;
  convenience ctor `(Class, from, to)`.
- Multiple atoms per signature (either/or selections); multiple signatures
  per operation.
- Helpers: `forNucleotideSequences(min, max)` (matches any combination of
  lists + single sequences with total count in range),
  `forNucleotideAlignment(minSeqs, maxSeqs)`, `forNucleotideAlignments(...)`,
  `forNucleotideAndProteinSequences(minNuc, maxNuc, minPro, maxPro,
  allowSequencesInAlignments)`, `forMatching(Function<List<doc>,Boolean>)`
  for custom predicates.
- Examples: single sequence `(SequenceDocument.class, 1, 1)`; exporter
  `forNucleotideSequences(1, Integer.MAX_VALUE)`.

---

## Options (declarative parameter system)

`public class Options implements XMLSerializable` — the single parameter
mechanism for operations, annotation generators, graphs, assemblers, print
dialogs, and standalone dialogs. The platform: **renders** a GUI from the
schema, **persists** values across invocations, **validates** on OK, and
exposes everything **programmatically**.

### Construction modes

1. Subclass `Options` (required to customize panel layout) and add options in
   the constructor, keeping typed accessors — the dominant SDK pattern:

```java
private static class MyOptions extends Options {
    private final BooleanOption trimEnds;
    private final IntegerOption nBases;
    MyOptions() {
        trimEnds = addBooleanOption("trimEnds", "Trim Ends", false);
        nBases   = addIntegerOption("numberOfBasesToTrim", "# bases", 5);
        nBases.addDependent(trimEnds, true);       // enabled only when checked
        nBases.setDisabledValue(0);                // value when disabled
    }
}
```

2. Use an `Options` instance directly and read via
   `options.getValue("name")` / `getValueAsString("name")`.

### Option types (`add*` factory methods)

| Factory                                                                         | Type / value                                                                                                                                  |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `addBooleanOption(name, label, default)`                                        | `BooleanOption`                                                                                                                               |
| `addStringOption(name, label, default[, placeholder])`                          | `StringOption`                                                                                                                                |
| `addMultipleLineStringOption(name, label, default, rows, monospaced)`           | `MultipleLineStringOption`                                                                                                                    |
| `addIntegerOption(name, label, default[, min, max])`                            | `IntegerOption`                                                                                                                               |
| `addDoubleOption(name, label, default[, min, max])`                             | `DoubleOption`                                                                                                                                |
| `addComboBoxOption(name, label, T[]                                             | List<? extends T> values[, default])`                                                                                                         |
| `addEditableComboBoxOption(name, label, default, values)`                       | editable combo                                                                                                                                |
| `addRadioOption(name, label, values, default, alignment)`                       | `RadioOption<T>` (`HORIZONTAL_ALIGN` etc., `DependentPosition` placement)                                                                     |
| `addDateOption(name, label, default)`                                           | `DateOption`                                                                                                                                  |
| `addColorChooserOption(name, label, defaultColor)`                              |                                                                                                                                               |
| `addFileSelectionOption(name, label, default[, save/open modes])`               | `FileSelectionOption`                                                                                                                         |
| `addExecutableFileSelectionOption(...)`                                         | executable picker                                                                                                                             |
| `addButtonOption(name, label, buttonLabel[, icon, callback])`                   | `ButtonOption`                                                                                                                                |
| `addLabel(label[, centre, spanWidth])`, `addLabelWithIcon`, `addDivider(label)` | layout                                                                                                                                        |
| `addHelpButton(dialogTitle, msg)` / `addHelpButtonOption`                       |                                                                                                                                               |
| `addChildOptions(name, label, description, Options[, hidden, pageChooser])`     | nested groups                                                                                                                                 |
| `addChildOptionsPageChooser(name, label, pages)`                                | tabbed pages                                                                                                                                  |
| `addCollapsibleChildOptions(...)`                                               | collapsible section                                                                                                                           |
| `addMultipleOptions(name, Options template, ...)`                               | user-replicable option groups (+/− buttons)                                                                                                   |
| `addCustomOption(OptionType)` / `addCustomComponent(JComponent                  | ComponentCreator)`                                                                                                                            |
| `addDocumentSelectionOption(name, label, ...)`                                  | `DocumentSelectionOption` — pick documents from the database (`FolderOrDocuments`, `OptionValueCreator`, `DoNotRememberInPreferences` marker) |
| `addPrimerSelectionOption(...)`                                                 | primer picker                                                                                                                                 |
| `addServiceOption(name, label, WritableDatabaseService...)`                     | database/folder chooser                                                                                                                       |
| `PasswordOption` (separate class)                                               | password entry                                                                                                                                |

### Per-option behaviors (`Options.Option`)

- `getValue()` / `setValue(...)`, typed subclasses with typed values.
- `setAdvanced(true)` — hidden behind "show more options".
- `setDescription(...)` (tooltip), `setHelp(...)`.
- `addDependent(parentOption, requiredValue[, position])` — enable/disable
  chaining (on `Option`, `BooleanOption`, `RadioOption`, and child-options
  dependencies).
- `setDisabledValue(...)` — value reported while disabled.
- `addChangeListener(SimpleListener)` — react to value changes.
- `setVisible/setHidden`, `setSpanningComponent`, `setFillHorizontalSpace`,
  `setWarningMessage`, `setProOnly`.

### Options-level behaviors

- Layout: `beginAlignHorizontally()` / `endAlignHorizontally()`, or override
  `createPanel()` / `createAdvancedPanel()` in a subclass.
- Validation: `verifyOptionsAreValid()` runs on OK; show messages if invalid.
- Persistence: `savePreferences()` / `restorePreferences()` (user defaults
  across runs), `restoreDefaults()`, `valuesToXML()` / `valuesFromXML()`.
- Full serialization: `Options` itself is `XMLSerializable` (labels,
  dependencies, everything) — this is how operation history re-runs recreate
  dialogs.
- Programmatic driving: `getDescriptionAndState()` lists names/values;
  `setValue(key, value)` / `setStringValue(key, string)` set them (this is
  how one plugin invokes another).
- **Threading**: not thread safe. May be built on any thread, but once
  displayed, programmatic value changes must happen on the Swing thread
  (values mirror to widgets immediately).

### `Options.OptionValue`

Base class for combo/radio values: `(name)` or `(name, value)`; subclass to
carry payloads. XML-serializable. Used e.g. for genetic-code tables in the
BackTranslation example.

---

## GeneiousActionOptions (UI placement declaration)

`public class GeneiousActionOptions` — builder-style (setters return `this`).

- Content: `name`, `description`, `Icons` (16/24/32), `Category`,
  `MainMenu` location(s), `KeyStroke` shortcut (+ `setOverrideShortcut` with
  `ShortcutChangeListener`), `Badge`, pro-only flag.
- Placement: `setMainMenuLocation(MainMenu)`, `setInMainToolbar(true)`,
  `setInPopupMenu(true)`, positions as doubles (`getMainMenuPosition()`,
  `getMainToolbarPosition()`, `getPopupMenuPosition()`, `DEFAULT_POSITION`).
- Submenus: `createSubmenuActionOptions(parentOptions, childOptions)` +
  `addSubmenuDivider(position)`.
- Identity: `getIdentifier()` = name+description concatenation (stable across
  renames); `getSearchName()`, `getLocationAsHtml()`.

`MainMenu` enum: `NotPresent`, `File`, `New` (under File), `Collaboration`,
`Edit`, `View`, `Tools` (complex actions), `Sequence` (simple actions on
sequences), `AnnotateAndPredict`, `Grid`, `Import` (File|Import), `Export`
(File|Export), `Help`, `ToolBar_Export`, `ToolBar_Add`,
`ToolBar_Export_Other`, `ToolBar_Add_Other`.

`Category` enum (role-based dispatch): `None`, `Alignment`, `TreeBuilding`,
`GenomeVsGenomeAlignment`, `StructureAlignment`. The user picks _the_
implementation per category; workflows call `getCategoryOperation(category)`.

---

## SequenceAnnotationGenerator

Generates (or edits) annotations on selected sequences inside the sequence
viewer; the viewer manages **undo/redo and saving automatically**.

- `getActionOptions()`, `getHelp()`, `getSelectionSignatures()`,
  `getUniqueId()`.
- `getOptions(documents, SelectionRange)` — `SelectionRange` is the user's
  current selection region (with `SelectionGrabOption` to let the generator
  control selection behavior).
- Simple contract:

```java
List<List<SequenceAnnotation>> generateAnnotations(
    AnnotatedPluginDocument[] docs, SelectionRange selection,
    ProgressListener progress, Options options);
```

One `List<SequenceAnnotation>` per input document.

- Advanced contract: `generate(...)` returns
  `AnnotationGeneratorResult` objects which can also:
  - delete annotations,
  - adjust residues via `ResidueAdjustment` (insertions/deletions/replacements
    relative to the original),
  - set document field values,
  - report results on alignments (`AnnotationGeneratorResultOnAlignment`).
- `SingleSequenceResultGenerator` — convenience base for one-sequence cases.
- Hidden qualifiers can exclude bookkeeping annotations from result counts
  (`HIDDEN_QUALIFIER_EXCLUDE_FROM_ANNOTATION_RESULT_COUNTER`).

---

## AlignmentOperation

The platform handles everything except the algorithm: input document types
(single sequences, lists, alignments), reordering/reversing, local vs global
mode, referenced documents, and characters the algorithm can't handle.

```java
List<CharSequence> align(SequenceDocument.Alphabet alphabet,
    List<CharSequence> sequences, Options options,
    ProgressListener progressListener);   // same order in, aligned out
```

- `supportsAlphabet(Alphabet)`, `getSupportedCharacters(alphabet)` (null =
  all), `isProOnly()`, `getTabPosition()` (ordering in the aligner chooser),
  `getName()`, `getUniqueIdPrefix()`.
- Options: `getOptions(alphabet, pairwise, isSingleAlignment)` or
  `getOptions(InputProperties)` (richer: counts, lengths, types).
- `getScore(alignedSequences, options)` → score stored on the resulting
  `DefaultAlignmentDocument`.
- `preserveSequenceOrder(options)`, `isRefine(options)` (refine existing
  alignment), `canRunOnGeneiousServer()/Local()`.
- Gap construction idiom (SDK example): rows are built with
  `SequenceCharSequence.withTerminalGaps(leading, seq, trailing)` — the
  aligner only needs to decide where gaps go.

---

## Assemblers

```java
void assemble(Options options, AssemblerInput input,
    ProgressListener progress, Assembler.Callback callback);
```

### Declaration

- `getUniqueId()` (+ `REFERENCE_UNIQUE_ID_SUFFIX` / `DE_NOVO_UNIQUE_ID_SUFFIX`
  when one implementation serves both modes), `getName()`, `getShortName()`,
  `getBadge()`, `getMenuPosition()`.
- Capabilities: `getReferenceSequenceSupport()` ∈ { `NoReferenceSequence`,
  `SingleReferenceSequence`, `MultipleReferenceSequences` };
  `getContigOutputSupport()` ∈ { `ContigsOnly`, `ConsensusOnly`,
  `ContigsAndConsensus` }; `providesUnusedReads()`, `providesUsedReads()`,
  `providesUsedReadsIncludeMates()`; `handlesTrimAnnotations()`;
  `supportsInputContigs()`, `supportsSingleInputContig()`;
  `supportsReferenceSequencesWithDuplicateNames()`;
  `canRunOnGeneiousServer()/Local()`; `isProOnly()`.
- `getOptions(OperationLocationOptions, AssemblerInput.Properties)`.

### AssemblerInput (streaming input model)

- `getNumberOfReferenceSequences()`, `getReferenceSequence(i, progress)`,
  `getReferenceSequenceReference(i)`.
- `getReads()` → `Reads` — **single-pass iterator**: `hasNext()`,
  `getNextReadPair()` → `Read` with `getReadNormalized()`,
  `getMateNormalized()` (null if unpaired),
  `getExpectedMateDistanceNormalized()` (sign adjusted so left read positive).
- Libraries: `getLibraries()` (`Library` = one sequence list/alignment doc +
  its data type), `getMergedLibraries(progress)`.
- Types: `DataType` (sequencing technology), `PairedDataType` (paired-end /
  mate-pair kind).
- Flags: `isGenerateContigs()`, `isSaveUnusedReads()`, `isSaveUsedReads()`,
  `isSaveUsedReadsIncludeMates()`, `isSetReferencedDocumentsOnContig()`,
  `getMaximumNumberOfContigsToGenerate()`, `getSampleName()`,
  `fractionOfDataToUse` support.
- Static counting helpers over documents (sequences in lists/alignments).

### Assembler.Callback (streaming output)

- `addContigDocument(SequenceAlignmentDocument contig, ...)` — deliver a
  contig as it is built.
- `addUnusedRead(Read, progress)` / `addUnusedReads(SequenceListDocument,
  progress)`.
- Consensus-sequence variants when `ConsensusOnly`/`ContigsAndConsensus`.
- Contig construction idiom (SDK example): build gapped rows with
  `SequenceCharSequence.withTerminalGaps` +
  `SequenceUtilities.createSequenceCopyAdjustedForGapInsertion`, then
  `new DefaultAlignmentDocument(rows, null, null, name)` + `setContig(true)`
  - `setContigReferenceSequenceIndex(0)` + `setMates(...)`, tracked during
    assembly with `PairedReadManager`.

---

## Workflows — composing other plugins

The SDK's `ExampleWorkflow` (`DocumentOperation` with empty selection
signature) shows the full pattern:

1. `CompositeProgressListener` with weights (0.8/0.15/0.05).
2. Get a service by id: `(DatabaseService)
   PluginUtilities.getGeneiousService("NCBI_nucleotide_gbc")` →
   `retrieve(Query.Factory.createQuery(text), progress)`.
3. Get the configured aligner:
   `PluginUtilities.getCategoryOperation(Category.Alignment)` →
   `getOptions(sequences)` → `setValue("operation", "MUSCLE_NUCLEOTIDE_")` →
   `performOperation(...)`.
4. Same for `Category.TreeBuilding` with
   `setValue("treeBuilding.buildMethod", "UPGMA")`.
5. Rename/return results; platform saves them.

---

## Mab notes

- The `Options` system is arguably the most transferable design: a typed,
  serializable, renderable, persistable parameter schema that doubles as the
  inter-plugin invocation protocol. In Rust: serde-schema structs + a UI
  renderer + per-field metadata (label, advanced, dependencies, disabled
  value). Dependencies-between-fields and disabled-values are the fiddly 20%
  that make it ergonomic — plan for them.
- The operation threading contract (background thread, no instance state,
  cancel via progress, null-or-exception result) maps to async Rust with a
  cancellation token passed everywhere; keep "platform saves results" as the
  default so operations stay pure producers.
- Selection signatures = capability matching; a Rust trait/enum describing
  accepted input cardinality/type could be checked at compile time for the
  common cases.
- Role-based categories (Alignment, TreeBuilding) + unique-id lookup together
  give both user choice and programmatic addressing.
- Streaming input (`Reads` single-pass iterator) and streaming output
  (callback) for assemblers is the right shape for large data in Rust too —
  resist materializing read sets.
