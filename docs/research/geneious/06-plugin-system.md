# 06 — Plugin System & Lifecycle

Packages: `...publicapi.plugin` (`GeneiousPlugin`, `PluginUtilities`,
`TestGeneious`, `DocumentType`).

Geneious implements almost all functionality as plugins (the sequence viewer,
local database, NCBI access, aligners are all plugins). First-party and
third-party plugins use the identical API.

---

## GeneiousPlugin — the entry point

`public abstract class GeneiousPlugin` — one class per plugin, instantiated
via a **public no-arg constructor**, then `initialize(...)` is guaranteed to
be the first call.

### Identity & metadata

| Method                              | Notes                                                                    |
| ----------------------------------- | ------------------------------------------------------------------------ |
| `getName()`                         | UI/log name                                                              |
| `getDescription()`                  | Short purpose text                                                       |
| `getHelp()`                         | HTML or plain text help                                                  |
| `getAuthors()`                      |                                                                          |
| `getVersion()`                      | Numeric dots; compared numerically segment by segment                    |
| `getMinimumApiVersion()` → `String` | Compared against the full API version string (e.g. `"4.11"`)             |
| `getMaximumApiVersion()` → `int`    | Compared against the **major** number only (e.g. `4` works with any 4.x) |
| `getIcons()`                        | `Icons` set                                                              |
| `getEmailAddressForCrashes()`       | Crash reports suspected to come from this plugin                         |
| `getPluginLicenses()`               | `List<License.PluginLicense>`                                            |
| `PLUGIN_NAME_LOCAL_DOCUMENTS`       | Constant: name of the Local Documents plugin                             |

### Extension point getters (the complete surface)

Each returns an array (empty default implementations provided):

| Getter                                  | Extension type                                                                        | Doc                                                                      |
| --------------------------------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `getServices()`                         | `GeneiousService[]` (incl. `DatabaseService`s)                                        | [05](05-storage-query.md)                                                |
| `getDocumentOperations()`               | `DocumentOperation[]`                                                                 | [07](07-operations-options.md)                                           |
| `getDocumentActions()`                  | `DocumentAction[]` — like operations but cannot create documents                      | [07](07-operations-options.md)                                           |
| `getSequenceAnnotationGenerators()`     | `SequenceAnnotationGenerator[]`                                                       | [07](07-operations-options.md)                                           |
| `getAlignmentOperations()`              | `AlignmentOperation[]`                                                                | [07](07-operations-options.md)                                           |
| `getAssemblers()`                       | `Assembler[]`                                                                         | [07](07-operations-options.md)                                           |
| `getDocumentViewerFactories()`          | `DocumentViewerFactory[]`                                                             | [08](08-viewers-ui.md)                                                   |
| `getSequenceGraphFactories()`           | `SequenceGraphFactory[]`                                                              | [08](08-viewers-ui.md)                                                   |
| `getSequenceViewerExtensionFactories()` | `SequenceViewerExtension.Factory[]`                                                   | [08](08-viewers-ui.md)                                                   |
| `getTreeViewerExtensionFactories()`     | `TreeViewerExtension.Factory[]`                                                       | [08](08-viewers-ui.md)                                                   |
| `getDocumentFileImporters()`            | `DocumentFileImporter[]`                                                              | [09](09-io-formats.md)                                                   |
| `getDocumentFileExporters()`            | `DocumentFileExporter[]`                                                              | [09](09-io-formats.md)                                                   |
| `getDatabaseFolderImporters()`          | `DatabaseFolderImporter[]` — import whole folders as database structures              | [09](09-io-formats.md)                                                   |
| `getDocumentTypes()`                    | `DocumentType[]` — static metadata (icons, names) for custom `PluginDocument` classes | [01](01-document-model.md#custom-document-type-example-textfiles-plugin) |
| `getPluginPreferences()`                | `List<PluginPreferences>` — tabs in the Geneious preferences window                   |                                                                          |

Dynamic plugins: `addPluginChangedListener(SimpleListener)` +
`firePluginChangedListeners()` let a plugin change its provided operations at
runtime.

### Dependencies

`getDependencies()` → list of plugin identifiers this plugin depends on. The
platform initializes dependencies first; a plugin must not assume other
plugins exist during `initialize` (all plugins initialize simultaneously).

---

## Lifecycle & threading rules

- Construction: empty constructor → `initialize(pluginUserDirectory,
  pluginDirectory)`.
- `pluginDirectory` — where the plugin's jar + bundled resources live;
  **may be read-only at runtime** (multi-user installs); null for internal
  plugins.
- `pluginUserDirectory` — guaranteed writable, unique per user; may not exist
  yet (`mkdir` it); the place for persistent plugin data. Temp files go
  through `FileUtilities.createTempFile` instead.
- **Threading**: Geneious installs plugins on a non-AWT thread; _no Swing/UI
  code may run in any `GeneiousPlugin` method_ during install. Operations
  run on background threads ([07](07-operations-options.md)); UI work goes
  through `ThreadUtilities.invokeAndWait` / `CallSoon`
  ([08](08-viewers-ui.md)).

---

## Packaging & distribution

### `.gplugin` format

A `.gplugin` file is a **zip**:

```
MyPlugin.gplugin (zip)
└── com.example.myplugin.MyPlugin/      # folder named EXACTLY the fully-qualified
    ├── MyPlugin.jar                    #   GeneiousPlugin class name; contains
    │   ├── classes...                  #   compiled classes + plugin.properties
    │   └── plugin.properties
    ├── some-dependency.jar             # extra jars: auto-added to classloader
    └── data-files, executables, ...    # resources visible via pluginDirectory
```

- Simpler form (used by the SDK examples): a single jar containing classes +
  `plugin.properties`, renamed to `.gplugin` (no folder).
- Folder form is required when bundling resources/dependencies.
- Classloader isolation: each plugin gets its own classloader; every jar in
  the plugin folder is added to it. Known workaround for libraries that use
  the thread context classloader:

```java
ClassLoader old = Thread.currentThread().getContextClassLoader();
try {
    Thread.currentThread().setContextClassLoader(getClass().getClassLoader());
    // call problematic library
} finally {
    Thread.currentThread().setContextClassLoader(old);
}
```

### `plugin.properties` (required in the jar)

```properties
plugin-name=com.biomatters.helloworld.HelloWorldPlugin   # FQCN of the GeneiousPlugin class
short-plugin-name=HelloWorld                              # used for the .gplugin file name
```

### Build system (SDK examples)

- Ant `build.xml` per example; classpath = `../GeneiousFiles/lib/{GeneiousPublicAPI.jar, jdom.jar, jebl.jar}`;
  `javac source/target 11`; `build` target jars classes + `plugin.properties`;
  `distribute` target renames `.jar` → `.gplugin` (or zips a folder for
  folder-type plugins).
- `copyPluginAndRename` ant target scaffolds a new plugin by renaming an
  example (classes, packages, build files).
- Eclipse `.launch` files run/debug plugins inside a Geneious instance.
- The Phobos PDF walkthrough (`PhobosPluginDevelopment.pdf`) drives the whole
  flow from an example project to a wrapped command-line tool.

---

## Registry & inter-plugin invocation (`PluginUtilities`)

String-keyed service locator for everything plugins provide:

| Lookup                                                            | Notes                                                                                                                                       |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `getGeneiousService(uniqueId)` / `getGeneiousServices()`          | Services/databases by id                                                                                                                    |
| `getDocumentOperation(uniqueId)`                                  | Operations by `getUniqueId()`                                                                                                               |
| `getSequenceAnnotationGenerator(uniqueId)`                        | Generators by id                                                                                                                            |
| `getCategoryOperation(Category)`                                  | The platform's chosen operation for a category (e.g. `Category.Alignment` → whichever aligner the user configured, `Category.TreeBuilding`) |
| `importDocuments(File, progress)` / `exportDocuments(File, docs)` | Round-trip through whatever importer/exporter matches                                                                                       |
| `doAlignment(sequences, progress)`                                | Prompt user to align with any installed aligner                                                                                             |
| `displayDocumentSearchDialog(...)`                                | Standard document-picker dialog over local/connected databases                                                                              |
| `installPlugin(File gplugin)`                                     | Programmatic install                                                                                                                        |
| Root-service listeners                                            | `addWritableDatabaseServiceRootListener(...)` etc.                                                                                          |

Because every operation declares its `Options` schema
([07](07-operations-options.md)), invoking another plugin is:
lookup → `getOptions(docs)` → inspect via `getDescriptionAndState()` →
`setValue/setStringValue(...)` → `performOperation(...)`. Example from the
SDK (`ExampleWorkflow`): search NCBI via a service, then run the category
alignment operation with `options.setValue("operation", "MUSCLE_NUCLEOTIDE_")`,
then the tree-building category operation with
`options.setValue("treeBuilding.buildMethod", "UPGMA")`.

---

## Testing (`TestGeneious`)

Bootstrap the platform inside a test JVM without launching Geneious:

- `TestGeneious.initialize()` — required before most API calls (e.g.
  `DocumentUtilities.createAnnotatedPluginDocument`).
- `initializeAllPluginsInClasspath()` — load all plugins so
  `PluginUtilities` lookups work.
- `initializePlugins(GeneiousPlugin... | String...)` — load specific plugins.
- `isRunningTest()` / `setRunningApplication()` — mode detection.

---

## Mab notes

- The "one registry object exposing N typed getters" is a fine extension
  model; in Rust each extension type would be a trait object collected from
  plugin manifests. Keep the empty-default behavior (absent capability =
  empty list, never null).
- API versioning via min/max ranges (full-string min, major-only max) is a
  compact compatibility contract worth adapting for a Mab plugin ABI.
- Separate _install dir_ (read-only) from _user data dir_ (writable) — this
  prevents a whole class of deployment bugs.
- String-keyed lookup is convenient but untyped; Mab can keep ids for
  persistence/UI while exposing typed handles internally.
- Category indirection (`getCategoryOperation`) decouples workflows from
  specific implementations (user picks the aligner once; workflows call "the
  aligner"). Mab should have the same notion of _role-based_ operation
  dispatch.
