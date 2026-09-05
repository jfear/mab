---
id: ADR-0007
title: Define SequenceDocument with Flat Core, Nested Metadata, and Content-Derived Uid
status: accepted
date: 2026-09-07
supersedes: ~
superseded-by: ~
tags: [domain-model, api-design, documents, metadata]
---

## Context

Mab's first document type is a biological sequence with its metadata —
the primary object parsed from FASTA, GenBank, and EMBL and stored in the
application. The existing `crates/mab-core/src/document.rs` stub defers
field design to this ADR.

## Decision

Define `SequenceDocument<A: Alphabet>` with a flat core (`uid: Uuid`,
`name`, `sequence: Sequence<A>`, `topology`, `annotations`) plus one
nested `SequenceMetadata` struct of `Option`-typed source-derived
fields (`description`, `accession`, `organism`, `genetic_code`,
`taxonomy`) and an `extras: BTreeMap` escape hatch for lossless import.
The `uid` is a **content-derived v5 UUID** computed at creation via
`Uuid::new_v5(MAB_NAMESPACE, sequence.as_bytes())` — identity is a pure
function of sequence content; import provenance is a storage-layer
concern. Internal annotation coordinates are **0-based
half-open**; derived statistics (length, GC%) are methods, not fields;
dates and persistence concerns live in a future storage envelope.

## Alternatives Considered

- **Flat core + one nested metadata** (chosen) — core stays lean; optional
  fields are grouped for diff/serialize; matches the split IGV/JBrowse use.
- **Fully flat** — bloated surface; hard to evolve the optional set.
- **Schema-declared field registry** (Geneious-style) — heavy for our
  scope; `extras` gives a lossless escape hatch without machinery.
- **Random (v4/v7) uid + separate accession** — re-imports silently
  duplicate; v5 makes identity a function of content.

## Consequences

- ✅ FASTA and GenBank import is lossless; `extras` catches every
  format-specific leftover.
- ✅ Same sequence → same uid; duplicate imports are impossible.
- ✅ Alphabet homogeneity enforced via ADR-0006's `<A: Alphabet>`.
- ❌ Editing the sequence produces a new uid; identical sequences share
  a uid regardless of import source (dedup by design) — distinguishing
  them, if ever needed, is future work (edit-chain / source-identity
  concepts).

## AI Guidance

When working in this area, an agent should:

- **Preserve:** The content-derived uid contract; 0-based half-open
  internal coordinates; `Alphabet` homogeneity; `extras` as the lossless
  import escape hatch.
- **Avoid:** Adding `molecule_type`, `created`, or `modified` fields
  (alphabet typing carries molecule kind; dates are storage-layer); storing
  derived statistics (length, GC%) as fields.
- **Prefer:** The `extras` map over new typed fields for format-specific
  leftovers; derived methods over stored caches.
- **Ask before changing:** `SequenceMetadata` shape, uid derivation
  strategy, or annotation coordinate convention — all are public API.

## Links

- Supersedes: ~
- Related: ADR-0006
