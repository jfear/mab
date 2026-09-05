# 02 — Feature data model & configuration system

Source: JBrowse 2 v4.3.0, commit 83ac4507cf.

TL;DR: A feature is a tiny duck-typed interface (`get(name)`, `id()`, `parent()`, `toJSON()`) over a plain string-keyed attribute map; the wire format is one flat JSON object per feature (`SimpleFeatureSerialized`) with nested `subfeatures`. Configuration is an MST model built from a declarative slot table where each slot is a value-union (value | `"jexl:…"` callback string); reads evaluate callbacks per-feature, and a CSS-like cascade (track value → session-wide promoted default → `promotedBase`) resolves "promotable" slots. Coordinates are **0-based half-open** everywhere downstream of parsing.

---

## 1. The `Feature` contract

`packages/core/src/util/simpleFeature.ts` — `interface Feature`:

| member       | signature                                                                                                                                                                                          | role                                                                                        |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `get`        | overloaded: `refName→string`; `name/type/id/source→string?`; `start/end→number`; `phase→0\|1\|2?`; `strand→-1\|0\|1?`; `score→number?`; `subfeatures→Feature[]?`; open `get(name:string): unknown` | single accessor for everything; anything not in the overload list is just a data-map lookup |
| `id()`       | `string`                                                                                                                                                                                           | unique identity (adapter-minted, unique per render result)                                  |
| `parent()`   | optional, `Feature?`                                                                                                                                                                               | parent handle (subfeatures only)                                                            |
| `children()` | optional, `Feature[]?`                                                                                                                                                                             | subfeatures (exons/CIGAR blocks/deletions)                                                  |
| `toJSON()`   | `SimpleFeatureSerialized`                                                                                                                                                                          | the RPC/serialization form                                                                  |

Only `start`, `end`, `uniqueId`, and (for subfeatures) `refName` are truly required. Everything else — type, source, score, strand, phase, arbitrary attributes — is optional and carried in a generic `Record<string, unknown>` data map. `tags(): string[]` lists which keys exist (used by the details panel and filter UIs).

Reserved/conventional fields: `uniqueId`, `refName`, `start`, `end`, `strand` (−1/0/1; subfeatures inherit the parent's strand when unset), `phase` (0|1|2, GFF3-style), `score`, `name`, `type`, `source`, `description`, `parentId`, `subfeatures`, `mate` (paired-end: `{refName, start, end, …}`), `aliases` (refName-alias records may omit coordinates entirely — the only legal coordinate-less feature). `__jbrowsefmt` is a reserved key holding formatter output (see §6).

**Coordinate convention:** 0-based, half-open `[start, end)`, interbase — same as BED/BAM. Format parsers convert: VCF's 1-based inclusive `POS` becomes `start = POS - 1` (`plugins/variants/src/VcfFeature/index.ts`); GFF3's 1-based inclusive columns likewise arrive converted by `@gmod/gff`/`gff-nostream`. SimpleFeature validates `end >= start` and rejects NaN. Nothing anywhere is 1-based after parsing; there is no region-relative arithmetic across the RPC boundary (worker output is absolute genomic positions).

### `SimpleFeature` (packages/core/src/util/simpleFeature.ts)

The default concrete implementation: holds `data: Record<string, unknown>`, inflated `subfeatures?: Feature[]`, `parentHandle?: Feature`, `uniqueId: string`. Subfeatures are inflated eagerly at construction (a nested array of plain args becomes child SimpleFeatures with ids `${parent.uniqueId}-${i}`); strand inheritance is resolved in `get('strand')` at read time rather than copied at construction. Effectively immutable — the constructor never mutates the caller's input.

### Serialization: `SimpleFeatureSerialized`

```ts
interface SimpleFeatureSerializedNoId {
  [key: string]: unknown     // generic attribute map — the whole model is open
  parentId?: string
  start: number; end: number; refName: string
  type?: string; strand?: number; name?: string
  id?: string | number; uniqueId?: string
  __jbrowsefmt?: Record<string, unknown>
  mate?: { refName: string; start: number; end: number; [k: string]: unknown }
  subfeatures?: SimpleFeatureSerializedNoId[]
}
interface SimpleFeatureSerialized extends SimpleFeatureSerializedNoId { uniqueId: string }
```

`toJSON()` spreads the whole data map, adds `uniqueId`, `parentId` (if parented), bakes inherited strand into the copy, and recursively serializes subfeatures. `SimpleFeature.fromJSON(json)` reconstructs — this is exactly what crosses the worker→main RPC boundary: a flat JSON tree per feature, no class instances, no schema. Note the JSON form is _flat attributes + nested `subfeatures` array_, not a generic key/value pair list.

**JEXL proxy** (`jexlFeatureProxy`, same file): config callbacks read features as plain properties (`feature.score`), via a Proxy forwarding every property to `get()`, `uniqueId`→`id()`, `parent`→re-wrapped parent. `buildJexlContext(args)` is the single context-construction path so callback variable sets never differ between config evaluation and filter chains.

### Alignment features (brief; full coverage is another chapter)

BAM/CRAM adapters do not use `SimpleFeature` for the render path: `plugins/alignments/src/BamAdapter/BamSlightlyLazyFeature.ts` implements the same `Feature` interface over a lazy `BamRecord` (same overload set, plus lazy `mismatches` and file-level mate refName ids) and only materializes `toJSON()` for the details path. CIGAR-derived aligned blocks/deletions surface as `get('mismatches')`/per-base arrays rather than as `subfeatures`. Other `implements Feature` classes in-tree: `plugins/gff3/src/Gff3Feature.ts`, `plugins/variants/src/VcfFeature/index.ts`, `plugins/maf/src/MafFeature.ts`, `plugins/legacy-jbrowse/src/NCListAdapter/NCListFeature.ts`.

---

## 2. Format features: two case studies

### Gff3Feature (`plugins/gff3/src/Gff3Feature.ts`)

Wraps a parsed GFF3 record whose column-9 attributes are **still raw text**, resolving each attribute only when `get(name)` asks (via `gff-nostream`'s `getAttribute`). Motivation: the render path reads a fixed handful of keys (`start`, `end`, `strand`, `type`, `phase`, `subfeatures`, `name`, `id`, `source`, `score`, `gbkey`) and packs them into typed arrays without the features ever crossing RPC; the details panel re-fetches the region separately to get everything. A fixed `switch` serves the eight parsed columns; the `default` branch does the (potentially costly) attribute parse. `toJSON()` materializes columns + all parsed attributes + inherited strand + `uniqueId` + recursive subfeatures (`parentId` included). Children are wrapped lazily (`${uniqueId}-${i}` ids), unlike SimpleFeature's eager inflation.

Adds over the base contract: `source` (column 2), `phase` (column 8), GFF3 attribute vocabulary (`ID`, `Name`, `Alias`, `Note`, `Dbxref`, `Ontology_term`, …) as string/string[] values.

### VcfFeature (`plugins/variants/src/VcfFeature/index.ts`)

Wraps an `@gmod/vcf` `Variant` + header parser. Fixed fields are converted in `dataFromVariant`: `refName=CHROM`, `start=POS-1`, `end=getEnd(variant, start)` (REF-length, or INFO `END` for symbolic ALTs), plus derived `type`/`description` from Sequence-Ontology terms (`getSOTermAndDescription` — SNP/indel/SV classification) and `name=ID.join(',')`. Extra accessors beyond the base contract, all lazily evaluated off the raw record:

| field       | type                      | notes                                            |
| ----------- | ------------------------- | ------------------------------------------------ |
| `REF`       | `string?`                 | reference allele                                 |
| `ALT`       | `string[]?`               | symbolic ALTs (`<DUP>`, `BND`) included verbatim |
| `QUAL`      | `number?`                 | VCF quality, read as `score`                     |
| `FILTER`    | `string \| string[]?`     |                                                  |
| `INFO`      | `Record<string, unknown>` | header-typed INFO fields                         |
| `genotypes` | `Record<string, string>`  | sample name → GT string                          |
| `samples`   | per-sample structure      | full FORMAT fields per sample                    |

`toJSON()` = `uniqueId` + variant JSON + converted data + materialized `samples`. `processGenotypes`/`processFormatFields` expose streaming callbacks for multi-sample renderers (the multi-sample display path interns genotype codes instead — see MULTI_SAMPLE_VARIATIONS in agent-docs).

**Pattern for Mab:** each format feature is a thin lazy façade over the raw record implementing one uniform interface; conversion to canonical 0-based coordinates happens once, at construction; format-specific vocabulary (genotypes, GFF3 attributes) is reached through the same open `get()` rather than a widening interface.

---

## 3. Configuration system (`packages/core/src/configuration/`)

### Mental model

A config schema (`configurationSchema.ts`, `ConfigurationSchema(name, definition, options)`) is a declarative table compiled into an MST model. Every entry in the table is one of exactly three kinds:

1. **Slot definition** (`ConfigSlotDefinition`) — a typed, default-carrying value that may alternatively hold a `"jexl:…"` callback string.
2. **Constant** — a bare string/number, becoming a volatile instance constant (read `model.someName`, never serialized as a slot).
3. **Sub-schema** — a nested `ConfigurationSchema(...)` type (or array/map of them), e.g. a track's `adapter` or `displays[]`.

Per-slot metadata (type, description, defaultValue, contextVariable) is _not_ on the instances: it lives in a **schema registry**, a `WeakMap<IAnyType, {definition, options}>` (`schemaRegistry.ts`), registered against both the inner model and the outer `stripDefault` wrapper so lookups succeed from either handle. There is no per-slot sub-model — the slot value is a plain property on the parent config node; `types.stripDefault` omits at-default values from snapshots.

### Slot definition shape

```ts
interface ConfigSlotDefinition {
  type: ConfigSlotType      // required; what marks a slot vs sub-schema
  defaultValue?: unknown    // REQUIRED for non-maybe* types; forbidden (must be absent) for maybe*
  description?: string
  model?: IAnyType          // custom MST value type (needed for the two enum types)
  contextVariable?: string[]  // callback params, e.g. ['feature'] — raises the jexl toggle in the editor
  advanced?: boolean        // hide behind "Show advanced settings"
  promotedBase?: unknown    // presence MAKES the slot promotable (see §4)
  validate?: (v: unknown) => boolean  // extra semantic check for promotable values
}
```

A slot's MST type is `types.union(JexlStringRefinement, valueModel)` — i.e. **value or `"jexl:…"` string**, stripped from the snapshot when equal to the default. Construction-time validation is thorough and throws (`configurationSlot.ts`): unknown type name; missing `defaultValue` on a non-maybe type; a concrete `defaultValue` on a maybe type (this happens through inheritance — a maybe override of a plain base slot inherits the base default via the spread merge; the repo's one case was `LinearMafDisplay.height`); a promotable slot that isn't a maybe type, has a default, declares `contextVariable` (callbacks are refused by the cascade, so the toggle would be a write sink), or whose `promotedBase` fails `isUsableValue`.

### Built-in slot types (`ConfigSlotType`, closed set)

| type                                                          | MST model                                                       | editor fallback | notes                                                                                                                       |
| ------------------------------------------------------------- | --------------------------------------------------------------- | --------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `string`, `text`                                              | string                                                          | `''`            | text is editor-presentation only                                                                                            |
| `integer`, `number`                                           | integer / number                                                | 1               |                                                                                                                             |
| `boolean`                                                     | boolean                                                         | true            |                                                                                                                             |
| `color`                                                       | string                                                          | `'black'`       | unvalidated CSS name/hex/jexl; widget picked off the type name                                                              |
| `stringArray`                                                 | array(string)                                                   | `[]`            |                                                                                                                             |
| `stringArrayMap`                                              | map(array(string))                                              | `{}`            |                                                                                                                             |
| `numberMap`                                                   | map(number)                                                     | `{}`            |                                                                                                                             |
| `fileLocation`                                                | `FileLocation` union (`{uri, locationType:'UriLocation'}` etc.) | placeholder uri | the on-disk/network pointer type                                                                                            |
| `frozen`                                                      | types.frozen()                                                  | `{}`            | arbitrary JSON object, e.g. `colorBy`                                                                                       |
| `maybeNumber` / `maybeBoolean` / `maybeColor` / `maybeFrozen` | maybe(…)                                                        | —               | `undefined` = "not set" / "decide from data"; the only reliable unset representation, since no config can spell `undefined` |
| `stringEnum` / `maybeStringEnum`                              | author-supplied `types.enumeration` via `model`                 | —               | the only two types with no builtin model                                                                                    |

The type set is closed and runtime-checked (`CONFIG_SLOT_TYPE_NAMES` Set) because JS plugins bypass tsc; a wrong name would otherwise silently degrade (e.g. a nonexistent `'enum'` renders as a free text box). `toCallbackValue`/`toFixedValue` convert between fixed values and `"jexl:" + JSON.stringify(v)` strings.

### Composition & inheritance

`ConfigurationSchema(name, definition, { baseConfiguration, explicitIdentifier, implicitIdentifier, explicitlyTyped, actions, views, extend, preProcessSnapshot })`:

- `baseConfiguration` must be the exact type `ConfigurationSchema()` returned (a `types.late` wrapper or union has no registered slot table and **throws** — it used to silently drop every inherited slot). The base's definition folds under the child's: **new slots are added; sub-schemas/constants are replaced wholesale; a redeclared slot merges field-by-field** (a spread, so omitting a key inherits it and stating `promotedBase: undefined` or `defaultValue: undefined` genuinely overwrites).
- `actions`/`views`/`extend` **compose as chains** (base first, then child — MST's `.actions()` per entry is what makes base members visible on `self`); `preProcessSnapshot` composes into `child(base(snapshot))`.
- Identifiers: `explicitIdentifier: 'trackId' | 'displayId'` makes a required MST identifier (and drives reference dispatch, §5); `implicitIdentifier` auto-generates via `ElementId`.
- `types.stripDefault` wraps the whole model, so an all-default sub-schema omits itself from its parent's snapshot — serialized configs stay minimal.
- Write guards: `setSlot(name, value)` throws on an undeclared name (ADR-052) with the valid slot list in the message; `setSubschema(name, data)` only accepts single sub-schema keys. Model actions beyond that: nothing — slots are plain assignments.

A generated snapshot test (`products/jbrowse-web/src/tests/ConfigSlotDefaults.test.ts`) pins every registered schema's slot defaults **and every enum slot's vocabulary** — dropping an enum member is a silent compat break because a saved session holding that value fails MST validation and the track fails to hydrate.

---

## 4. Reading config: `getConf`, `readConfObject`, callbacks, promotables

Three readers (`readConfObject.ts`), all existing to evaluate `"jexl:…"` on read:

- **`readConfObject(conf, slotPath?, args?)`** — off a **live MST config node** (passing a snapshot is a type error but can't be a runtime check; slots at default are _absent_ from snapshots). Single-slot reads are allocation-free; a path array drills through sub-configs, evaluating jexl only at the last segment. Sub-config values are returned as referentially-stable snapshots (frozen in dev) so downstream computeds can memoize. The jexl instance comes from the node's env (`pluginManager.jexl`, carrying plugin-registered functions). An **arg-less read of a jexl slot still evaluates**, against a context where every variable is `undefined` — deliberately, after being built, measured, and backed out.
- **`readConfigValue(config, key, feature, jexl)`** — plain-object variant for workers/renderers; jexl passed explicitly.
- **`readConfSlot(config, path, args, jexl?)`** — decides MST-vs-plain at runtime (dialogs handed either).

**`getConf(model, slotPath, args)` is exactly `readConfObject(model.configuration, slotPath, args)`** (`getConf.ts`) — sugar for the `.configuration` hop, equally strict on slot names at compile time (when the holder's schema is concrete; a widened `AnyConfigurationModel` switches the check off, which is why mixin casts must name concrete schemas per repo rules). `setConf(model, slot, value)` is the typed write counterpart over `setSlot`.

**Callbacks:** a slot value is a `"jexl:…"` string; on read it is compiled (memoized) and evaluated with `args` as variables — typically `{ feature }` (wrapped in the jexl proxy, §1) or `{ config }` for About dialogs. JEXL is a real expression language with plugin-registered functions (`pluginManager.jexl`); the serialized config carries the raw string, so callbacks survive `fullConfSnapshot` and are evaluated **in the worker per feature** via `readConfigValue`. This is the mechanism behind per-feature coloring, filters, and labels: the admin writes `jexl:jexl:get(feature,'type')=='mRNA' ? 'blue' : 'black'` in the slot, and the renderer resolves it per feature.

**Promotable slots** (`promotableSlots.ts`, `promotableResolve.ts`, `promotableDefaults.ts`, `promotablePin.ts`): declaring `promotedBase` makes a slot promotable — a user can pin a **session-wide default for all tracks of that display type**, and a track may follow it or customize. The cascade (`resolveConf(model, slot)` → `resolveSlotIn`), deliberately separate from `getConf` (ADR-046) and main-thread-only (it consults the session):

```
value = track's own stored value (if isUsableValue)      // "customized"
      else session-wide promoted default (if isUsableValue)
      else promotedBase                                   // CSS `initial`
```

- `undefined` (a maybe* type) is the only inherit sentinel; `promotedBase` is `initial`.
- `isUsableValue` (slotShape.ts) gates every tier: not a `jexl:` string, correct JS shape per type (finite number, enum membership, object/array shape for frozen), plus the slot's optional `validate` hook (e.g. a saved `colorBy` naming a since-removed color scheme degrades to base instead of crashing). Applied to `promotedBase` at construction too, so the bottom tier is usable by construction.
- All value comparisons are `deepEqual`, never `===` (MST reconstructs object values).
- `SlotResolution { base, promoted, customized, inherited, value }` drives the pin UI and the About dialog's copy-config note.
- **Serializing a display config across any boundary (worker, shared session) must flatten promotables first** — `fullConfSnapshot` throws on a raw promotable (`undefined` sentinel would ship). The public entry point is `getConfigSnapshotWithPromotables(display)`.

**Schema registry** (`schemaRegistry.ts`): `WeakMap` keyed on the MST type; `getConfigurationSchemaDefinition(node)` is the single "what are this config's slots?" accessor (used by the slot facade, promotable defaults, and `fullConfSnapshot`). "Is this a config schema" is registry membership, not a flag on the type.

---

## 5. Config snapshot format & persistence

- **In a session file / URL**: configs serialize through MST `getSnapshot`, i.e. **only non-default slot values** (stripDefault), the identifier (`trackId`/`displayId`), `type`, and nested sub-configs. A slot whose value is a callback serializes as the `"jexl:…"` string. Track configs referenced by views serialize to their `trackId` string; a view that synthesizes a track nobody else registered writes the **full inline config** instead (`TrackConfigurationReference`'s union-of-id-or-snapshot, `configurationSchema.ts`).
- **Frozen tracks + hydration**: `session.tracks` entries are `types.frozen` plain objects until referenced; the first reference hydrates via `schemaType.create(frozen)` through `TrackConfigurationReference`'s custom ref `get()`, cached per (PluginManager, schemaType, frozen object) WeakMap (ADR-031 — MST custom references aren't memoized). Hydration validates: an invalid config throws on first read, and `view.tracks` is kept free of unusable tracks at the entry points. Non-admin sessions get a per-track editable copy (ADR-032) so quick edits never mutate the shared frozen base.
- **`fullConfSnapshot(confObject)`** (`fullConfSnapshot.ts`): a plain-object snapshot including **all** values even at default — self-contained so an RPC worker can read it with no schema. Slots pass raw (`jexl:` strings included for per-feature evaluation); direct sub-configs recurse (arrays/maps of sub-schemas are silently dropped); constants skipped; throws if a nested schema declares promotable slots (the cascade only resolves top-level display slots).
- **Versioning**: no config-schema version field. Evolution is by (a) `preProcessSnapshot` migration hooks composed into schemas (e.g. display-stub injection for every registered displayType, legacy-key lifts), and (b) session-level migrations in the products. A saved value MST can't validate makes the whole track fail to hydrate — which is why enum vocabularies are pinned by test.
- `ConfigurationSnapshot<SCHEMA>` gives compile-time checking of config _literals_ (the `const` generic slot-name set, values `unknown`: slot type, `jexl:` string, or absent); it fires only on object literals (excess-property rule), not on `JSON.parse` results.

---

## 6. Feature detail widgets

`packages/core/src/BaseFeatureWidget/` — the generic "click a feature, see its attributes" panel.

**State model** (`stateModelFactory.ts`): `BaseFeatureWidget` MST model — fields: `id`, `type:'BaseFeatureWidget'`, `featureData` (formatted, frozen), `unformattedFeatureData` (raw `SimpleFeatureSerialized`, frozen), `view`/`track` (safeReferences — the widget can outlive its track), `trackId`/`trackType`, `maxDepth`, `sequenceFeatureDetails` (nested sub-model for sequence panels), `descriptions`; volatiles `error`, `sequenceHoverPosition`. An autorun applies formatting and stores the result; **`postProcessSnapshot` persists the formatted data as `finalizedFeatureData`** (unless >2 MB) so reopening a session does not re-run callbacks; `preProcessSnapshot` accepts legacy key names. Displays override `featureWidgetType` (hook) to point at specialized widgets (e.g. `VariantFeatureWidget` in plugins/variants), which share a drawer panel by widget `id`.

**How attributes become a display** (`formatDetails.ts`, `formatDetailsConfigSchema.ts`):

1. The raw serialized feature is stored as `unformattedFeatureData`.
2. `applyFormatDetails(tiers, featureData)` runs the `formatDetails` config schema — present both on the session (`configuration.formatDetails`) and per track, **two tiers**: the track's object spreads over the session's (`mergeFormatCallbacks`, which drops non-plain-object tiers and keeps `null`/`undefined` values — those are how a callback hides a field).
   - Slots: `feature` (frozen, `contextVariable:['feature']`, default `{}`) — returns an object of fields merged onto the feature; `subfeatures` — same, applied per subfeature down to `depth` (maybeNumber, default resolved to `DEFAULT_FORMAT_DETAILS_DEPTH = 2`, i.e. stops at transcripts); `maxDepth` (maybeNumber) — nesting the panel renders at all.
   - Output is stamped onto the feature copy as **`__jbrowsefmt`**, only where a callback produced something; the detail components spread `__jbrowsefmt` over the raw fields at render. New key = new row, existing key = rewritten, `undefined` = hidden, bare URL string = auto-linked.
   - Fast path: if neither tier _declares_ a callback (raw property check, not a reader — an arg-less jexl read would throw/junk per tier), the feature is returned unchanged; this matters because subfeature trees are huge (a RefSeq BRCA1: 368 transcripts, ~16k nodes) and unformatted tracks are the common case.
3. The identical mechanism exists for About dialogs: `formatAbout` (`formatAboutConfigSchema.ts`), same two-tier merge, callback variable is `config` (the track's configuration) not `feature`.

---

## Mab notes

- **Feature = flat attribute map + optional subfeatures + unique id.** A Rust equivalent: one struct `{ unique_id, ref_name, start: u32, end: u32, strand: i8, attrs: BTreeMap<String, AttrValue>, subfeatures: Vec<Feature>, parent_id: Option<...> }` — the open string-keyed map is load-bearing (every format's vocabulary, plus user jexl filters, hangs off it). Serialize exactly this shape for cross-thread transfer; no classes.
- **Fix coordinates once, at parse.** Convert VCF/GFF3/BAM 1-based inclusive to 0-based half-open in the parser; after that, everything downstream (layouts, renderers, stats) assumes half-open interbase. Keep a single interval type; reject `end < start` and NaN at construction.
- **Lazy vs eager feature parsing is a real design axis.** JBrowse keeps two feature classes for GFF3 (lazy façade for render, full materialization for details) because rendering reads ~8 keys per feature. In Rust, prefer parsing attributes on demand behind the uniform accessor, or store raw column text offsets.
- **Config = schema-checked tree of typed slots; callbacks are deferred strings.** The `"jexl:"` prefix trick — value-or-expression union in one field, evaluated on read with a feature context — is a clean pattern to steal for per-feature coloring/filtering in Mab (Rust analogue: a small expression-language enum in the slot value, evaluated in the worker). Worker reads config as plain data (`fullConfSnapshot` shape: all values explicit, callbacks raw) — worth replicating so the worker never needs the schema.
- **The "unset" sentinel matters.** JBrowse spends `undefined` on it via maybe* types; in Rust model it as `Option<T>` and never let a concrete default double as "inherit" — their subtlest config bugs all trace to that collision.
- **Session-wide display-type defaults (promotable slots) are a small CSS cascade**: own value → per-display-type session default → base. Cheap to implement, high user value; the non-obvious part is the `isUsableValue` gate that degrades stale/invalid saved values instead of failing.
- **Schema-derived validation with zero version numbers:** JBrowse evolves configs via preProcessSnapshot migrations and pins enum vocabularies by snapshot test. For Mab: generate a JSON Schema from the slot-table definition (the session-spec work in agent-docs already recommends exactly this) and keep migrations as pure snapshot→snapshot functions composed base-first.
- **Reserved-key discipline:** `uniqueId`, `__jbrowsefmt`, `parentId`, `subfeatures`, `mate` are reserved in the attribute namespace. In Rust, separate these from the attribute map in the struct rather than sharing one namespace (JBrowse's single map is a JSON-convenience artifact, and `__jbrowsefmt`-style scratch keys are its cost).
- **Ambiguities to re-verify in Mab design:** (1) `fullConfSnapshot` silently drops arrays/maps of sub-schemas — a format JBrowse hasn't needed, not a guarantee; (2) arg-less reads of jexl slots evaluate with all-undefined context (documented as fallout-producing); (3) session files may contain either track-id references or full inline configs for the same field — any reader must handle both.
