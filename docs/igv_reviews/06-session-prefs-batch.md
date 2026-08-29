# 06 — Sessions, Preferences, Batch Mode & Event Bus

Source: IGV v3.0.0-beta.4 (`src/main/java/org/igv/{session,prefs,batch,event}`). Packages: `org.igv.session`, `org.igv.prefs`, `org.igv.batch`, `org.igv.event`.

**TL;DR** — A session is a serializable view-state snapshot: genome id (or inline genome definition), locus/frames, ordered tracks with per-track render state, regions of interest, hidden attributes, and per-session preferences. IGV 3 writes JSON (legacy XML still readable); autosave writes JSON on a timer and at exit. Preferences are string key→value maps in named categories layered over compiled-in defaults from `/preferences.tab`, persisted to a single user file. Batch/scripting is a flat line-oriented command language executed by `org.igv.batch.CommandExecutor`, reachable from a file (`BatchRunner`) or a raw TCP socket (`CommandListener`, default port 60151). The event bus is a tiny class-keyed observer registry.

## 1. Session data model

### `org.igv.session.Session`

The in-memory session object. Key fields:

| Field                                                                         | Type                                        | Notes                                                                                                                                                                                                  |
| ----------------------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `version`                                                                     | int                                         | Session file format version (XML `version` attr; JSON writer emits implicit v8).                                                                                                                       |
| `path`                                                                        | String                                      | Where session was loaded from (base for relative paths).                                                                                                                                               |
| `locus`                                                                       | String                                      | Current view locus string (e.g. `chr8:128,747,872-128,748,032` or `egfr`).                                                                                                                             |
| `referenceFrame`                                                              | `ReferenceFrame`                            | Default frame; multi-locus views live in `org.igv.ui.panel.FrameManager`.                                                                                                                              |
| `regionsOfInterest`                                                           | `Map<String, Collection<RegionOfInterest>>` | Grouped by chromosome. `RegionOfInterest` start/end are **0-based, end-exclusive** (see comment in `RegionOfInterest.java`: "locations displayed to the user are 1-based. start and end are 0-based"). |
| `currentGeneList`                                                             | `org.igv.lists.GeneList`                    | Named list of loci; enables multi-frame "gene list mode".                                                                                                                                              |
| `hiddenAttributes`                                                            | `Set<String>`                               | Sample/track attribute names hidden from panels.                                                                                                                                                       |
| `preferences`                                                                 | `Map<String,String>`                        | Per-session preference overrides (distinct from user prefs).                                                                                                                                           |
| `colorScales`                                                                 | `Map<DataType, ContinuousColorScale>`       | Per-datatype default scales.                                                                                                                                                                           |
| `sampleFilter`, `nextAutoscaleGroup`, `dividerFractions`, `removeEmptyPanels` | —                                           | Misc view state.                                                                                                                                                                                       |
| `history`                                                                     | `History(100)`                              | Locus back/forward stack (below).                                                                                                                                                                      |

`Session` subscribes to `ViewChange` events and pushes history entries when `recordHistory` is set.

### `org.igv.session.Persistable`

Interface every persistable object (tracks, data ranges, filters) implements: `marshalXML` / `unmarshalXML(element, version)` / `unmarshalJSON(JSONObject)`. Tracks serialize themselves; the session readers/writers only orchestrate. Note asymmetry: no `marshalJSON` — JSON output is built by each track writing arbitrary keys into a `JSONObject` it is handed.

### `org.igv.session.SessionElement` / `SessionAttribute`

String constant tables for XML element and attribute names.

- **Elements**: `Session`, `Global` (legacy root aliases), `Resources`/`Resource`/`Files`/`DataFile`, `Panel`, `PanelLayout`, `Track`, `DataTracks`/`FeatureTracks` (panel wrappers), `DataRange`, `ColorScale(s)`, `Regions`/`Region`, `GeneList`, `HiddenAttributes`/`VisibleAttributes`/`Attribute`, `Filter`/`FilterElement`, `Preferences`/`Property`, `Frame`.
- **Attributes**: `genome`, `locus`, `version`, `name`, `id`, `path`, `type`, `index`, `clazz` (XML class name → Java class via `SessionElement.getClass`; short names map to `org.igv.track.DataTrack`, `FeatureTrack`, `SequenceTrack`, `DataSourceTrack`, `BlatTrack`, anything else via reflection `Class.forName`), `color`, `altColor` (as `"r,g,b"` 0–255 strings), `renderer` (e.g. `HEATMAP`, `BAR_CHART`), `height`, `visible`, `autoScale`, `colorScale` (encoded `ContinuousColorScale;min;minPositive;max;base;negColor;nullColor;posColor`), `start`/`end` (0-based), `windowFunction`, `displayMode`, `groupTracksBy`, `hasGeneTrack`, `hasSequenceTrack`, etc.

## 2. Session file formats

### JSON (current, writer `JSONSessionWriter` — only writer used by save/autosave)

Top-level object (see `test/sessions/json/*.json`):

```json
{
  "reference": { ... genome definition or id ... },
  "locus": "chr8:128750162-128751539",
  "nextAutoscaleGroup": "2",
  "sampleinfo": [ { "name": ..., "url": ..., "format": "sampleInfo" } ],
  "tracks": [ { "id": ..., "type": ..., "name": ..., ... per-track state ... } ],
  "roi": [ { "features": [ { "chr": ..., "start": <0-based int>, "end": <0-based int>, "description": ... } ] } ],
  "geneList": { "name": ..., "loci": ["chr1:1-100", ...], "frames": [ { "name","chr","start","end" } ] },
  "hiddenAttributes": ["attr1", "attr2"]
}
```

- **Genome**: either `"genome": "<id>"` (reader loads by id via `GenomeManager`) or `"reference": {...}` — a full `org.igv.feature.genome.load.GenomeConfig` serialized (`twoBitURL`, `chromSizesURL`, `cytobandURL`, `aliasURL`, `name`, `id`, `wholeGenomeView`, `chromosomeOrder: [...]`, `hubs: [...]`). The writer embeds `Genome.getConfig().toJSON()` minus its `tracks` key (session owns tracks).
- **locus**: single string (may be space-delimited multi-locus) or an array of locus strings.
- **tracks**: array of track objects; each is produced by the track's own key set. Common keys: `id`, `name`, `type` (`sequence`, `variant`, `merged`, `combined`, `blat`, `motif`, or data types), `url` or `path`, `format`, `order` (large float, `order` field from `org.igv.track.Track`), `height`, `visibilityWindow`, `color`, renderer settings. `merged` tracks nest child objects under `"tracks"`; `combined` tracks reference other tracks by id.
- **frames** in `geneList`: frame extents are `start`/`end` **in double "bip" coordinates, 0-based half-open** (from `ReferenceFrame.getOrigin()` / `getEnd()`).

### XML (legacy, reader `XMLSessionReader` ~1150 lines; writer code removed — only reads)

Root `<Session genome="hg19" locus="chr8:128747872-128748032" version="8" hasGeneTrack="false" hasSequenceTrack="true">` (legacy files may root at `<Global>`). Child inventory:

- `<Resources><Resource name="..." path="..." type="bam" index="..."/></Resources>` — resources are loaded **first and in parallel** (threads; synchronously when in batch mode or when the file has no `<Track>` elements), then tracks are allocated to panels by id. Relative paths resolved against the session file location.
- `<Panel height="2885" name="DataPanel" width="1133"><Track clazz="org.igv.track.DataSourceTrack" id="..." name="..." color="255,0,0" height="50" visible="true" renderer="HEATMAP" autoScale="false" colorScale="ContinuousColorScale;..."> <DataRange baseline="0.0" maximum="9.919" minimum="-7.8853" type="LINEAR"/> </Track></Panel>`
- `<Regions><Region chr="chr1" start="1000" end="2000" name="..." type="..."/></Regions>` — 0-based.
- `<HiddenAttributes><Attribute name="..."/></HiddenAttributes>` (or `VisibleAttributes` from which hidden = all − visible, `XMLSessionReader.java:588`).
- `<GeneList name="..."><Locus .../></GeneList>`.
- `<Preferences><Property name="..." value="..."/></Preferences>` — session-scoped prefs (`Session.setPreference`).
- `<Filter>` / `<FilterElement>` for sample filters; `<ColorScales><ColorScale type="..." value="..."/>`.

Track `clazz` values are resolved via `SessionElement.getClass` (short names or fully-qualified names via reflection). Readers chosen by `SessionReader.of` / `IndexAwareSessionReader`: JSON if extension `.json`/`.igv` content or object shape; XML otherwise; `UCSCSessionReader` handles UCSC-style `browser`/`track` line files. Genome change during load: if session genome id ≠ current, `GenomeManager.loadGenomeById` is called and the session is reset.

### Load/save lifecycle

- Save: `JSONSessionWriter.saveSession(session, file)` — pure string dump of `createJsonFromSession`. `CommandExecutor` `savesession` forces `.json` extension. `Constants.SESSION_RELATIVE_PATH` historically toggled relative paths (now only XML legacy).
- Load: reader parses, loads genome first, then sampleinfo, then loads resources concurrently and matches loaded tracks to their JSON descriptors by (in order) name → id → type → format → single-unclaimed (`JSONSessionReader.java:378-430`), then sets `order`, calls `track.unmarshalJSON`, and adds to panels. Sequence/genome annotation tracks are pre-loaded from the genome and re-ordered around session tracks.
- On session change: `IGV.resetSession(path)` — clears tracks/panels, replaces `Session` object (fresh `History`), fires genome events as needed.

### History — `org.igv.session.History`

- `Entry { locus: String, zoom: int }`; two structures: `allHistory` (append-only log) and `activeStack` (LinkedList with `currPos` index for back/forward).
- `push(locus, zoom)` skips: empty strings, gene-list-mode entries not starting with `"List"`, and duplicates of the top entry. Back/forward walks `activeStack` and re-executes loci via `SearchCommand`; `"List: <name>"` entries restore a gene list; `CHR_ALL` restores whole-genome view.
- Cap 100 entries; recorded automatically from `ViewChange` events with `recordHistory=true`.

### Autosave — `org.igv.session.autosave`

- `SessionAutosaveManager` writes to `DirectoryManager.getAutosaveDirectory()`:
  - **Exit autosave**: single file `exit_session_autosave.json` (overwritten), controlled by pref `AUTOSAVE_ON_EXIT` (default TRUE); optional reload at startup via `AUTOLOAD_LAST_AUTOSAVE` (default FALSE).
  - **Timed autosaves**: `session_autosave<ISO-8601-timestamp>.json`, one per tick of `AutosaveTimerTask` (`AUTOSAVE_FREQUENCY` minutes, default 10; `AUTOSAVES_TO_KEEP` max files, default 0 = disabled; oldest-by-name deleted when over the limit). Failure disables timed autosave until restart.
- All autosaves are JSON (`JSONSessionWriter`).

## 3. Preferences — `org.igv.prefs`

### Architecture

- `PreferencesManager` (static singleton): manages a `Map<String category, IGVPreferences>`. Three categories: `Constants.NULL_CATEGORY = "NULL"` (global), `THIRD_GEN` (3rd-gen alignment defaults), `RNA`. RNA/THIRD_GEN inherit from NULL via a parent `IGVPreferences` chain.
- **Defaults** come from a packaged TSV resource `/preferences.tab` parsed by `loadPreferenceList()`: lines `#<TabLabel>\t<category>` start groups, `##<group>` sections, data lines are `KEY\t<label>\t<type>\t<default>\t[group]` (2-token lines = hidden prefs). Defaults loaded through `loadDefaults23()` + the table.
- **User prefs** persist to `DirectoryManager.getPreferencesFile()` — a properties-like text file where lines starting `##` switch category, others are `KEY=VALUE`. Overridable at launch via `setPrefsFile` / `loadOverrides(path)`.
- **Lookup chain** (`IGVPreferences.get`): user prefs → parent user prefs → defaults → parent defaults. Typed accessors `getAsBoolean/getAsInt/getAsFloat/getAsColor/getAsColorScale` with per-key caches; unknown/invalid values warn and fall back (0/false/null).
- Legacy key aliases (`SAM>SORT_OPTION`→`SAM.SORT_OPTION`) and value translations (e.g. `SAM.SHADE_BASE_QUALITY` "quality"→"true") applied at load.
- Session-scoped prefs (`Session.preferences`, XML `<Preferences>`) are separate and override nothing in `PreferencesManager`.
- Changes publish `org.igv.prefs.PreferencesChangeEvent` on the event bus; `PreferencesManager` is an observer and reacts (e.g. toggling `CommandListener` when `PORT_ENABLED`/`PORT_NUMBER` change, `IGVPreferences.java:358`).

### Important keys (`org.igv.prefs.Constants`, ~250 keys — sample)

| Key (constant)                                                                             | Meaning / default                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `DEFAULT_GENOME`                                                                           | Genome id loaded at startup.                                                                                                                                                                                                               |
| `GENOMES_SERVER_URL`                                                                       | Genome server (`IGV.genome.sequence.dir`).                                                                                                                                                                                                 |
| `PORT_ENABLED` / `PORT_NUMBER`                                                             | Command listener; defaults TRUE / 60151.                                                                                                                                                                                                   |
| `AUTOSAVE_ON_EXIT` / `AUTOLOAD_LAST_AUTOSAVE` / `AUTOSAVE_FREQUENCY` / `AUTOSAVES_TO_KEEP` | TRUE / FALSE / 10 (min) / 0.                                                                                                                                                                                                               |
| `DEFAULT_VISIBILITY_WINDOW`                                                                | Max span (bp) for feature detail loading; beyond it, windowed aggregation is used.                                                                                                                                                         |
| `SEARCH_ZOOM`                                                                              | Zoom level after search/goto.                                                                                                                                                                                                              |
| `FLANKING_REGION`                                                                          | bp flanking on feature goto.                                                                                                                                                                                                               |
| `SAM_*` (~80 keys)                                                                         | Alignment rendering: `SAM.QUALITY_THRESHOLD`, `SAM.SAMPLING_WINDOW`, `SAM.MAX_LEVELS` (downsample count), `SAM.SORT_OPTION`, `SAM.GROUP_OPTION`, `SAM.COLOR_BY`, `SAM.SHOW_SOFT_CLIPPED`, filter flags, base-mod colors `BASEMOD.*_COLOR`. |
| `COLOR.A/C/G/T/N`                                                                          | Nucleotide colors for sequence rendering.                                                                                                                                                                                                  |
| `CHART_AUTOSCALE`, `TRACK_HEIGHT_KEY`, `EXPAND_FEAUTRE_TRACKS` (sic)                       | Chart/track layout.                                                                                                                                                                                                                        |
| `RECENT_SESSIONS`, `RECENT_URLS`, `LAST_SNAPSHOT_DIRECTORY`, etc.                          | UI MRU state — persisted in the same prefs file.                                                                                                                                                                                           |
| `SESSION_RELATIVE_PATH`                                                                    | Legacy relative-path sessions.                                                                                                                                                                                                             |
| `CRAM_CACHE_*`                                                                             | Sequence cache dir/size.                                                                                                                                                                                                                   |

## 4. Batch / scripting — `org.igv.batch`

### `CommandExecutor` (~1300 lines)

Dispatch on the first whitespace-delimited token (case-insensitive, quoted strings preserved via `StringUtils.breakQuotedString`). Returns `"OK"` or an error string. Command set (params shown as `p1 p2 ...`):

| Command                                                                                                                                                                                                                                                                                                                                                                               | Semantics                                                                                                                                            |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `load` / `loadfile <path> [name=x ...]`                                                                                                                                                                                                                                                                                                                                               | Load data file(s)/URL; extra args as key=value (e.g. index URL, coverage merging).                                                                   |
| `genome <id-or-path>`                                                                                                                                                                                                                                                                                                                                                                 | Load genome by id or file/URL. `currentGenomePath` queries it.                                                                                       |
| `goto` / `gotoimmediate <locus>`                                                                                                                                                                                                                                                                                                                                                      | Search/gotion to locus string (1-based display coords, e.g. `chr1:1,000-2,000`, gene name, or `chr` for whole chromosome).                           |
| `addframes <locus>`                                                                                                                                                                                                                                                                                                                                                                   | Add a frame (multi-locus view).                                                                                                                      |
| `region <name> <chr> <start> <end> [desc]`                                                                                                                                                                                                                                                                                                                                            | Define a region of interest; `start`/`end` parsed as 0-based (`Locus`-style parsing; see `RegionOfInterest` — internal 0-based, displayed 1-based).  |
| `snapshot [filename] [region]`                                                                                                                                                                                                                                                                                                                                                        | Write PNG/JPEG (optionally of `trackpanels` region) to `snapshotdirectory`.                                                                          |
| `snapshotdirectory <path>`                                                                                                                                                                                                                                                                                                                                                            | Set snapshot output dir.                                                                                                                             |
| `savesession <file>`                                                                                                                                                                                                                                                                                                                                                                  | Save as JSON (extension forced to `.json`).                                                                                                          |
| `new` / `reset` / `clear`                                                                                                                                                                                                                                                                                                                                                             | Fresh session.                                                                                                                                       |
| `sort <option> [locus] [reverse]`                                                                                                                                                                                                                                                                                                                                                     | Sort alignment tracks (`base`, `quality`, `sample`, `readGroup`, `insertSize`, `firstOfPairStrand`, `mateChr`, `readOrder`, `readname`, `tag`, ...). |
| `group <option> [tag]`                                                                                                                                                                                                                                                                                                                                                                | Group alignments (`strand`, `sample`, `library`, `readGroup`, `base`, `insertion`, `selected`...).                                                   |
| `colorBy <option> [tag]`                                                                                                                                                                                                                                                                                                                                                              | Alignment coloring.                                                                                                                                  |
| `collapse`/`expand`/`squish [track]`                                                                                                                                                                                                                                                                                                                                                  | Display modes; track identified by name, alt name, or resource path.                                                                                 |
| `setdataRange <track> <min max                                                                                                                                                                                                                                                                                                                                                        | auto>`,`setLogScale <track> <bool>`,`setColor/setAltColor <track> <color>`                                                                           |
| `setTrackHeight <track> <h>`, `setRowHeight`, `maxpanelheight <px>`                                                                                                                                                                                                                                                                                                                   | Layout.                                                                                                                                              |
| `overlay`, `separate`, `renameTrack`, `remove <track>`                                                                                                                                                                                                                                                                                                                                | Track composition.                                                                                                                                   |
| `viewaspairs`, `samplingwindowsize`, `maxdepth`, `preference <key> <value>`, `version`, `zoomin/zoomout`, `tofront`, `setSleepInterval`, `setCredentials`/`clearCredentials`, `oauth`/`setaccesstoken`/`clearaccesstokens`, `sortByAttribute`, `fitTracks`, `showAttributes`, `showDataRange`, `tweakdivider`, `setSequenceStrand`, `setSequenceShowTranslation`, `toolsYaml`, `echo` | Misc.                                                                                                                                                |
| `exit`                                                                                                                                                                                                                                                                                                                                                                                | Quits the app.                                                                                                                                       |

Comments in script files: lines starting `#` or `//` skipped.

### `BatchRunner`

Reads a script file line-by-line, `setIsBatchMode(true)` → `Globals.setBatch` + `Globals.setSuppressMessages` (suppresses dialogs; also forces synchronous resource loading in the XML reader). Relative paths resolved from the script's directory (`rootPath`). Optional pre-loaded default genome if first command isn't `genome`. Resets max panel height afterward.

### `CommandListener` — the socket/REST interface

- Plain TCP server on `PORT_NUMBER` (default **60151**, `test/.../TestClient.java` connects to `127.0.0.1:60151`); started when `PORT_ENABLED` is true (default).
- Two modes: **socket protocol** — command lines terminated by newline, replies `OK` or error string; and **HTTP GET** — responds with `HTTP/1.1 200 OK` / `204 No Response`, URL-decodes the path, and supports parameters `file`, `bigDataURL`, `sessionURL`, `dataURL`, `hubURL`, `index` (base64-coded values via `Base64Coder`). This is what igv.js "backends" and igvtools use. Commands are executed via the same `CommandExecutor`.

## 5. Event bus — `org.igv.event`

`IGVEventBus`: map of event **class → set of observers** (`WeakHashMap`-backed sets so unsubscribed-observers can still be GC'd; `unsubscribe` still recommended). `post` dispatches synchronously on the caller thread to observers of the exact event class (no class-hierarchy matching). Not a singleton — `IGVEventBus.getInstance()` is the default bus, e.g. Sashimi plots create private buses. No threading guarantees: `post` is unsynchronized (sub/unsub are).

Event types (all tiny marker/data classes implementing `IGVEvent`):

| Event                                                        | Fields / role                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ViewChange` (nested `Type {ChromosomeChange, LocusChange}`) | `chrName`, `start`/`end` (double, **0-based half-open** frame coords), `recordHistory`, `panning`. Factory methods distinguish _Cause_-style vs _Result_ style — comments in `ViewChange`: Cause events change the model (`ReferenceFrame`), Result events are posted after change so UI repaints. |
| `GenomeChangeEvent` / `GenomeResetEvent`                     | Genome loaded or reset — drives track rebuilds and renderer caches.                                                                                                                                                                                                                                |
| `DataLoadedEvent`                                            | Data finished loading for a track (repaint trigger).                                                                                                                                                                                                                                               |
| `RefreshEvent`                                               | Generic repaint/refresh request.                                                                                                                                                                                                                                                                   |
| `TrackSelectionEvent`, `TrackFilterEvent`, `TrackGroupEvent` | Track list mutations (select, filter, regroup).                                                                                                                                                                                                                                                    |
| `AlignmentTrackEvent`                                        | Alignment-track-specific state changes (sort/group/color).                                                                                                                                                                                                                                         |
| `StopEvent`                                                  | Request long-running tasks to stop.                                                                                                                                                                                                                                                                |
| `PreferencesChangeEvent` (`org.igv.prefs`)                   | Preference key/value changed.                                                                                                                                                                                                                                                                      |

Note: "track added/removed" has no dedicated event — track-list changes are propagated by explicit calls into `IGV`/`TrackPanel` and repaint events, not the bus.

## Mab notes

- **Session = pure view state** decoupled from data. The JSON schema (reference config or genome id; locus; ordered track descriptors with per-track render state; ROI array; hidden attributes; geneList with frames) is a good starting shape for a Mab session format. Keep tracks self-serializing via a `Persistable`-like trait so new track types don't touch the session reader/writer.
- **Robust track-matching on load is the hard part**: one resource URL can fan out into multiple tracks (BAM → coverage + alignment + junction). IGV matches JSON descriptors to loaded tracks by name/id/type/format/sole-unclaimed and dedupes futures per URL. Design Mab's loader around explicit descriptors (e.g. `url + sub-id` keyed futures) instead of post-hoc name matching.
- **Coordinates**: ROI and frame extents are 0-based half-open internally; display and locus strings are 1-based inclusive. Make the locus-string parser accept both and standardize on 0-based half-open in the domain model.
- **History** is a simple bounded locus stack with dedup and gene-list-mode suppression — cheap to replicate; store `(locus_string, zoom)` rather than full frame state.
- **Autosave**: two distinct files (exit + timed ring) written as JSON, retention by count. Rust equivalent: `tempdir`/config dir + timestamped files, no locking needed.
- **Preferences**: a layered string map (user → category → defaults compiled from a table) with typed accessors and caching is simple and effective. Consider serde-typed structs per category in Rust instead of string maps, but keep a string-map escape hatch for forward compatibility of the prefs file.
- **Command language** is flat, line-oriented, and trivially scriptable; keeping a similar text command set (load/genome/goto/region/snapshot/savesession/sort/group/exit) plus a TCP listener gives Mab headless/CI testing for free. Parsing to an enum of commands with typed args is a clean Rust design.
- **Event bus**: class-keyed synchronous dispatch is easy in Rust with an enum of events + `HashSet` of observer channels (or a `tokio::sync::mpsc` broadcast). Distinguish "cause" vs "result" locus events as IGV does — it cleanly separates model mutation from repaint, which maps well onto command/mutation → state diff → render pipelines.

Ambiguities: the `region` batch command's coordinate convention is not documented at the call site (parsed via generic locus parsing; internal ROI is 0-based); `exit_session_autosave.xml` filename lingers in `getExitSessionAutosaveFile`/`getTimedSessionAutosaveFiles` filters while writes use `.json` (legacy-name mismatch); session `version` handling in JSON is implicit (writer emits nothing, reader ignores it); `order` values in JSON use very large sentinel floats (`-9007199254740991`).
