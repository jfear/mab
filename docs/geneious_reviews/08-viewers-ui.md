# 08 — Viewers & UI Extension Points

Packages: `...publicapi.plugin` (`DocumentViewer`, `SequenceGraph`,
`SequenceViewerExtension`, `TreeViewerExtension`),
`...publicapi.components` (`Dialogs`, widgets),
`...publicapi.utilities` (`CallSoon`).

> **Heads-up:** this layer is the most Java/Swing-specific of the API. For
> Mab (Iced), treat it as a catalog of _concepts and contracts_, not
> implementation patterns.

---

## DocumentViewer (renders documents)

`public abstract class DocumentViewer` — one instance per opened document
tab; created by a `DocumentViewerFactory`.

| Member                                                        | Role                                                                             |
| ------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `getComponent()` → `JComponent`                               | **The only abstract method** — the widget to display                             |
| `getName()`                                                   | Tab name                                                                         |
| `getActionProvider()`                                         | Hook for copy/paste and custom actions (`ActionProvider.getCopyAction()` etc.)   |
| `getExtendedPrintable()` / `getPrintable()`                   | Printing + "save to image" (prefer Extended)                                     |
| `getPrintableComponent()`                                     | Alternate component for printing                                                 |
| `getFindable()`                                               | Ctrl+F support contract                                                          |
| `getNoLongerViewedListener()`                                 | Notified when the tab closes (`noLongerViewed(isTemporary)`) — release resources |
| `getIncomingMessageHandler()` / `getOutgoingMessageHandler()` | Inter-viewer messaging (below)                                                   |
| `ViewerLocation` (nested)                                     | Where the viewer is shown                                                        |

### ExtendedPrintable (paginated output)

- `getOptions(isSavingToFile)` → `Options` shown in the print dialog.
- `print(Graphics2D, Dimension pageSize, pageIndex, Options)` →
  `PAGE_EXISTS`/`NO_SUCH_PAGE`.
- `getRequiredWidth(options)`, `getRequiredHeight(width, options)`,
  `getPagesRequired(dimensions, options)` — also used by save-to-image.
- `ExtendedPrintable.Factory` for viewer factories that supply printing.

### DocumentViewerFactory

```java
DocumentViewer createViewer(AnnotatedPluginDocument[] docs);
DocumentSelectionSignature[] getSelectionSignatures();
String getName()/getHelp()/getDescription();
```

- Contract: documents handed to `createViewer` are **guaranteed loaded** —
  `getDocumentOrCrash()` is safe inside.
- `ViewPrecedence` — ranks multiple viewers that match the same selection.
- Platform behavior: displays the viewer whenever the user selects documents
  matching the signature.

---

## SequenceGraph (tracks-with-numbers inside the sequence viewer)

`public abstract class SequenceGraph` — a graph lane rendered in the sequence
viewer (identity, conservation, hydrophobicity, sequence logos, coverage).

### Factory

`SequenceGraphFactory`:

- `createResidueBasedGraph(Alphabet, isAlignment, isChromatogram, isContig)`
  → graph or null (context filtering; e.g. "alignments only, not contigs").
- Also sequence-based factory variants.
- `DefaultSequenceGraphFactories` helpers for the common case:
  `createSingleSequenceGraphFactory(name, description, showForAlignments,
  defaultHeight, alphabet, scorer, isLineGraph)` with
  `SingleSequenceScorer { Double getScore(char residue); Color
  getColor(double score); }` and `SequenceAlignmentScorer` (per-column
  scores).

### Graph contract

- Data handoff: `setResidues(List<CharSequence> sequences,
  List<NucleotideGraph> graphs, boolean ignoreEndGaps)` — the viewer supplies
  (aligned) rows.
- Rendering: `draw(GeneiousGraphics2D g, int startResidue, int endResidue,
  int startX, startY, endX, endY, double averageResidueWidth,
  int previousSectionWidth, int nextSectionWidth,
  int previousSectionResidueCount, int nextSectionResidueCount)` — draw one
  visible section; section boundaries are given so multi-section drawing is
  seamless.
- `getDefaultLocation()` → `Location` enum: `ABOVE_RESIDUES` /
  `BELOW_RESIDUES`; `getDefaultHeight()`, `drawScaleBar(...)`, `getCursor()`.
- Performance metadata: `getApproximateCalculationWorkRequiredPerResidue()`
  (scheduling hint).
- `SequencePropertyRetriever` (nested) — extra info about sequences for
  graph computations.
- Graphs are `MouseListener`/`MouseMotionListener`s — interactive.
- `GeneiousGraphics2D` wraps `Graphics2D` (coordinate scaling for HiDPI etc.).

---

## SequenceViewerExtension (panels/stats in the sequence viewer)

`SequenceViewerExtension` + `Factory` / `StatisticsFactory`:

- `StatisticsFactory.createStatistics(PropertyRetrieverAndEditor,
  progress)` → `List<StatisticsSection>` — computed for the current
  selection; cancellable.
- `StatisticsSection`: id, numeric value, display text, `verticalPosition`
  (constants like `POSITION_SEQUENCE_LENGTH` +
  `RELATIVE_POSITION_DIRECTLY_BELOW` offsets), and optional
  `DocumentFieldAndValue` — when provided, the statistic **becomes a column
  in the document table** for all sequences.
- `PropertyRetrieverAndEditor` — accessor/editor facade over the viewed
  document: current selection (`getSelectionForStatistics()`),
  `getSequenceCharSequence(sequenceIndex)`, annotation wrappers
  (`SequenceAnnotationWrapper`), state frequencies (`StateFrequencies`),
  component access; `DiscardReferencesCallback` for releasing memory.
- Non-statistics extensions add toolbar buttons/panels via
  `ComponentLocation`.
- SDK pattern (statistics): iterate selection intervals → sequence range →
  clip residue interval to `[leadingGaps, trailingGapsStart)` before scanning
  to avoid walking gigagaps in big contigs.

---

## TreeViewerExtension (side panels in the tree viewer)

`TreeViewerExtension` + `Factory.createTreeViewerExtension(viewer, doc)`:

- `getPanel()` / `getPanelTitle()` — side-panel widget.
- Events **in**: `treeChanged(TreeChangeEvent)`,
  `selectionChanged(TreeSelectionChangeEvent)` (selected `Node` set).
- Events **out**: `fireTreeChanged(new TreeChangeEvent(tree))` (replace the
  tree, e.g. after adding node attributes), `fireSelectionChanged(...)`.
- Tree model is JEBL: `Tree`, `Node`, taxon lookup, per-node
  `setAttribute(name, value)`; copy trees with `Utils.copyTree(...)` before
  mutating.

---

## Inter-viewer messaging

`DocumentViewerMessageHandler` — viewers looking at the same documents can
talk:

- Platform assigns the outgoing handler via
  `DocumentViewer.setOutgoingMessageHandler(...)`; broadcast with
  `getOutgoingMessageHandler().handleMessage(...)`.
- Receiving: return a handler from `getIncomingMessageHandler()`; it gets
  `handleMessage(Element message, String senderName, boolean focusReceiver)`.
- Specialized: `setSequenceSelection(SequenceSelection, senderName)` —
  selection sync between viewers (e.g. tree tip ↔ sequence row).
- Messages are XML elements — schema-free, sender-identified.

---

## Platform UI utilities (Swing-specific but instructive)

- `CallSoon` (`utilities`): schedule a `Runnable` on the EDT —
  `add(runnable)`, `add(runnable, delayMillis, overrideExisting[,
  EnabledProvider])`, `cancel(runnable)`. Coalescing + delay make it the
  standard "redraw soon, but not 60× per second" mechanism.
- `ThreadUtilities.invokeAndWait(...)` — used by background operations that
  must prompt the user (documented on `performOperation`).
- `Dialogs` (`components`): platform dialogs built on `Options` and a
  `DialogOptions` enum — `showOptionsDialog(options, title,
  restoreAndSavePreferences)`, `showNonModalOptionsDialog(...)`,
  `showDialog(...)`, `showYesNoDialogWithRememberMyPreference(preferenceKey,
  ...)`, `showOkDialogWithDontShowAgain(...)`, `showApplyToAllDialog(...)`.
  Note the recurring pattern: dialogs carry **preference keys** for
  "remember/don't show again" state.
- `components` package widgets: `GTextField` etc. (look-and-feel-consistent
  controls); `laf` package for theming; `StandardIcons` for shared icon
  vocabulary.
- Printing infrastructure is options-driven and paginated (see
  `ExtendedPrintable`) — same `Options` system as operations.

---

## Mab notes

- Transferable contracts: factory + selection signature for viewers; "docs
  guaranteed loaded" invariant at creation; explicit
  `noLongerViewed` resource-release hook; capability-style printing contract
  (page geometry queries + render per page).
- Sequence graphs as "data handoff then draw visible section" is a good
  rendering model for Iced too: give the graph the rows once, ask it to draw
  rectangles [start..end] as the viewport scrolls.
- Statistics sections that optionally promote to document-table columns is a
  nice pattern: computed-on-selection values share the `DocumentField`
  schema.
- `CallSoon`-style coalesced scheduling corresponds to Iced's message
  batching/debouncing; keep the "coalesce + optional delay + cancel"
  semantics for viewport-driven recomputation.
- Dialogs persisting "don't show again" via preference keys: worth adopting
  as a generic confirm-dialog API.
