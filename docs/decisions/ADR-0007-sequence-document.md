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

Mab needs an immutable domain object for sequences imported from FASTA, GenBank, and EMBL.
The model must preserve useful metadata and annotations without mixing persistence concerns into `mab-core`.

## Decision

Define `SequenceDocument<A: Alphabet>` with private fields for a content-derived `Uuid`, name, validated `Sequence<A>`, topology, annotations, and nested `SequenceMetadata`.
Derive the UUID with v5 over normalized residue bytes only; alphabet, metadata, and provenance are excluded, so identical bytes across alphabets share an identifier.

Use a reusable 0-based half-open `Interval` with `start < end`.
`AnnotationInterval` adds partial-end flags.
Store annotation intervals in source/traversal order, validate overlap through a sorted view, and represent circular origin spans as two ordinary intervals for now.
Store qualifiers as `Vec<(String, String)>`, sorted by key/value with exact duplicates removed.

Keep additional document-level key/value metadata in `SequenceMetadata::extras`.
Compute length and GC fraction on demand.
Dates, provenance, editing, serialization, and complete INSDC location expressions remain future work.

## Consequences

Imports can retain the modeled metadata and annotation subset, and repeated content is detectable.
The shared interval type centralizes coordinate semantics.
UUID alone is not an alphabet-specific storage key.
Complete source-format fidelity requires a later I/O/location ADR.

## Links

- Related: ADR-0005, ADR-0006
