# JBrowse 2 — Deep Architectural Reviews

Agent-friendly reference documentation distilled from the JBrowse 2 source
code (`jbrowse-components`), for use while designing and building Mab. The
focus is on **concrete structures, attributes, storage mechanics, and
viewport/session state** — the parts most useful for designing Mab's own
data model in Rust. TypeScript/React/MobX-State-Tree implementation
specifics are kept brief; each doc ends with Mab notes flagging what
transfers.

## Repository map

JBrowse 2 is a pnpm-workspace monorepo of three kinds of workspaces:

```
jbrowse-components
├── packages/                     shared libraries (no UI product of their own)
│   ├── core                      THE foundation: PluginManager, configuration
│   │                             system, data adapters base, RPC, assembly
│   │                             manager, session/utils, text search manager
│   ├── render-core               rendering backend HAL (WebGPU→WebGL2→Canvas2D),
│   │                             RenderLifecycleMixin, displayPhase, SVG export
│   ├── shader-tools              .slang → generated TS shader compiler
│   ├── display-kit               display mixins (MultiRegionDisplayMixin,
│   │                             GlobalFetchMixin, RegionTooLargeMixin, chrome)
│   ├── display-ui                shared display UI components
│   ├── alignments-core           coverage compute/SNP coverage/consensus
│   ├── wiggle-core               wiggle data types + score/scale utilities
│   ├── synteny-core              comparative-view fetch state + dotplot utils
│   ├── sv-core                   structural-variant/breakend plumbing
│   ├── ld-core, cigar-utils,     LD matrix utils; CIGAR parsing; MM/ML base
│   │   modifications-utils       modification parsing; track-add forms and
│   ├── add-track-core            the master format-guessing table
│   ├── text-indexing,            trix index building (+ convenience lib)
│   │   text-indexing-core
│   ├── product-core, app-core,   shared product/session bases (web-core,
│   │   web-core, embedded-core   embedded JBrowse, React products)
│   └── tree-sidebar              track/tree selector UI
├── plugins/                      feature bundles registered via PluginManager
│   ├── sequence, config          reference-sequence + assembly sidecar adapters;
│   │                             inline/FromConfig adapters, aliases, cytobands
│   ├── alignments                BAM/CRAM/SAM/htsget + pileup display (ch. 06)
│   ├── wiggle, gccontent         BigWig/multi-wiggle displays; GC content
│   ├── variants, gff3, gtf, bed  VCF, GFF3, GTF, BED/BedGraph/BEDPE adapters
│   ├── maf, hic, gwas, arc, blat multiple alignment, Hi-C, GWAS, arcs, BLAT
│   ├── comparative-adapters      PAF/PIF/chain/delta/mashmap/MCScan synteny
│   ├── linear-genome-view,       the view types (LGV, circular, dotplot,
│   │   circular-view, dotplot-   comparative, spreadsheet, breakpoint-split,
│   │   view, linear-comparative- sv-inspector)
│   │   view, spreadsheet-view,
│   │   breakpoint-split-view,
│   │   sv-inspector
│   ├── trix, text-indexing       text-search adapter + indexing RPC
│   ├── data-management, menus,   connections/hubs, UI chrome, jobs list,
│   │   jobs-management, canvas,  bookmarks, feature canvas tracks, auth
│   │   grid-bookmark,
│   │   authentication,
│   │   legacy-jbrowse, rdf       JBrowse 1 stores; SPARQL endpoints
└── products/                     deployable apps
    ├── jbrowse-web               the flagship web app (session model finalizer)
    ├── jbrowse-desktop           Electron app (local files, index jobs)
    ├── jbrowse-cli               config-editing CLI (add-assembly/track/…)
    ├── jbrowse-react-app,        React embeddables (LGView, CGView, build-
    │   jbrowse-react-*-view,     your-own); jbrowse-img headless SVG/PNG
    │   jbrowse-build-your-own,   renderer; jbrowse-capture Playwright
    │   jbrowse-img,              screenshots; aws deployment helpers
    │   jbrowse-capture, aws
```

## Sources

Everything here was derived from the **JBrowse 2 v4.3.0** source (paths below
are relative to the `jbrowse-components` repository root):

| Source                        | Path                                                                                                              | Used for                                                                                                                              |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| JBrowse 2 source (TypeScript) | `packages/`, `plugins/`, `products/`                                                                              | All type/structure details, verified against the code                                                                                 |
| Subsystem map                 | `agent-docs/ARCHITECTURE.md` + `agent-docs/reference/`                                                            | Authoritative mechanism deep-dives (fetch, GPU rendering, region-too-large, refName namespaces, session spec, SVG export, plugin ABI) |
| Per-directory guides          | `CLAUDE.md` files (root, `plugins/alignments/src/`, `plugins/wiggle/src/`, `packages/core/src/configuration/`, …) | Orientation and hot-path rules                                                                                                        |
| Version                       | package 4.3.0, git commit `83ac4507cf` (past tag `v4.3.0`)                                                        | Pinned for every claim in these docs                                                                                                  |

## Document index

| Doc                                                        | Contents                                                                                                                                                                                                                                        |
| ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [01-assembly-model.md](01-assembly-model.md)               | Assembly config schema + shorthand, lazy assembly lifecycle, `Region` type, refName aliasing/canonicalization and the two refName namespaces, sequence adapters, genetic codes                                                                  |
| [02-feature-config-model.md](02-feature-config-model.md)   | `Feature` contract + `SimpleFeatureSerialized` wire format, Gff3/Vcf feature case studies, the configuration slot system (17 slot types, JEXL callbacks, promotable-slot cascade), config snapshots, feature detail widgets                     |
| [03-plugin-system.md](03-plugin-system.md)                 | `Plugin`/`PluginManager`, the 10 pluggable element groups, extension points (fold/accumulate/notify), plugin loading + trust, track↔display↔adapter wiring, ABI stability                                                                       |
| [04-track-display-render.md](04-track-display-render.md)   | Track config vs track state, display mixin family, fetch autoruns + `rpcDataMap` staleness axes, three-backend render HAL (WebGPU/WebGL2/Canvas2D), shader workflow, display phases + region-too-large gate, wiggle case study, hover/selection |
| [05-adapters-data-loading.md](05-adapters-data-loading.md) | `BaseAdapter` contract, full adapter inventory table, RPC drivers + transferables, stop-token cancellation, adapter/chunk caching, byte-budget gating from index estimates, htsget, remote range-request filehandles                            |
| [06-alignments-model.md](06-alignments-model.md)           | BAM/CRAM/SAM/htsget adapters, the columnar `WorkerPileupData` DTO, coverage/SNP-coverage pipeline, downsampling/caps, MM/ML base modifications, sort/group/color tiers, GPU pass composition, SV arcs/sashimi, consensus calling                |
| [07-view-session-model.md](07-view-session-model.md)       | LinearGenomeView viewport math (bp-window state), blocks + visible regions, rubberband/overview, navigation actions + search dispatch, other view models, session model field inventory, session serialization/sharing/autosave                 |
| [08-io-formats.md](08-io-formats.md)                       | Master format→adapter table (index, queryability, coordinates), the `formats.ts` auto-detection table + loose track configs, trix text search, SVG/PNG/PDF exporters, `jbrowse-cli` commands                                                    |

## How to use these docs

- **Designing a Mab data structure** (genome, features, alignments): read
  01, 02, and 06. They describe the _shape_ of JBrowse's data, which is the
  best guide for Mab's Rust types.
- **Designing Mab storage/persistence**: read 05 (plus the sequence adapters
  in 01 and the session format in 07).
- **Designing Mab's track/render layer**: read 04 (and 07 for the viewport;
  06 for the alignments display, the most complex real-world example).
- **Adding a file format**: read 08 first (format → adapter/index/
  conventions), then the chapter owning that format's model.
- **Designing Mab's configuration system**: read 02 (slot model) and 03
  (registry/element wiring); JBrowse's config-as-data approach is the most
  directly portable subsystem.
- **Designing Mab's UI**: read 07; most of JBrowse's UI is React/MUI-
  specific and only the _concepts_ (viewport math, blocks, navigation,
  session layout) transfer to Iced.

## Conventions used in these docs

- Type names are JBrowse names, with their package/file path on first
  mention per section (e.g. `packages/core/src/assemblyManager/assembly.ts`
  `Assembly`). Paths are repo-relative to `jbrowse-components`.
- Coordinates: JBrowse's internal convention is **0-based, start-inclusive /
  end-exclusive (half-open, interbase)** everywhere after parsing; 1-based
  formats (GFF3, VCF, SAM POS, …) are converted at parse time, and the only
  re-conversion back is export (e.g. SAM `POS = start + 1`). Worker output
  is always **absolute genomic uint32** — never pixels, never
  region-relative. User-facing locStrings and tick labels are 1-based
  inclusive. Each doc states conventions explicitly wherever they appear.
- Each doc ends with a **Mab notes** section: short implications for the
  Rust implementation. These are opinions/starting points, not decisions.
- Ambiguities found in the source are noted in-line rather than papered
  over.

## Big-picture takeaways (structure focus)

1. **Everything is a pluggable element in a string-keyed registry.**
   Adapters, tracks, displays, views, widgets, RPC methods, connections,
   internet accounts — all registered by name into `PluginManager` group
   maps, and those type strings (`"AlignmentsTrack"`, `"BamAdapter"`) are
   the persistence/interchange format. Registries + phased install + a
   closed lifecycle is the whole plugin architecture (ch. 03).
2. **Configuration is data: declarative slot tables compiled to models.**
   Every slot is value-or-`"jexl:…"` callback, serialized minimally
   (defaults stripped), with a CSS-like cascade for session-wide defaults.
   Config snapshots are self-contained plain objects that workers can read
   without the schema (ch. 02).
3. **Fetch and render are separate stages with a strict data contract.**
   Adapters parse in RPC workers; results are immutable, absolutely-
   positioned typed arrays keyed by viewport region (`rpcDataMap`); the main
   thread uploads them once to GPU buffers and re-renders every frame —
   pan/zoom is a redraw, not a refetch. Every display also ships a Canvas2D
   draw fn that doubles as the SVG-export path (ch. 04).
4. **Invalidation axes are deliberately separated.** User settings
   (`rpcProps`, global), per-region content keys (`regionFetchKey`, e.g.
   zoom bin), presence (`regionHasData`), layout (main-thread rows), and
   recoloring are distinct tiers, and each setting declares which tier it
   invalidates. This is the cleanest caching design in the codebase
   (ch. 04, 06).
5. **Assemblies are tiny configs; everything else is lazily derived.**
   `name` + aliases + one sequence track, with regions, refName alias maps,
   and cytobands loaded on first observation and memoized. The one wart:
   refName spelling changes across the RPC boundary and only one direction
   has layer-level support (ch. 01).
6. **The adapter contract is small and gating is measured from indexes.**
   ~6 methods (`getRefNames`, `getFeatures`, `getHeader`, `getRegions`,
   `getRegionByteSize`, `getSequence`) cover ~45 adapters; "region too
   large" is decided from index chunk math before any feature download
   (ch. 05).
7. **The viewport is stored in bp space.** LGV persists
   `displayedRegions: Region[]` + a `windowStartBp`/`windowWidthBp` window;
   pixels (`bpPerPx`, `offsetPx`) are derived. Render chunks (static
   blocks, ~800 px) are decoupled from fetch windows (visible regions)
   (ch. 07).
8. **Alignments are columnar, not objects.** The worker→main payload is ~25
   parallel typed arrays keyed by read index plus per-event arrays with
   parent indices — clone-cheap, GPU-uploadable, and cache-friendly;
   coverage/frequency aggregates are precomputed in the worker and packed
   as fixed-stride GPU instance buffers (ch. 06).
9. **One ordered formats table drives all ingestion.** Filename regex →
   (adapter, index spec, track type), shared byte-for-byte by the browser's
   guesser, the CLI, and validation; loose configs (`{uri: …}`) expand on
   every ingestion path (ch. 08).
