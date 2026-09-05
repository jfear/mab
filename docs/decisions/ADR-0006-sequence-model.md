---
id: ADR-0006
title: Model Sequences as a Generic Struct Parameterized by a Marker Alphabet
status: accepted
date: 2026-09-07
supersedes: ~
superseded-by: ~
tags: [domain-model, api-design, sequences]
---

## Context

Mab needs a fundamental sequence type for nucleotides (DNA/RNA) and amino
acids, both stored as `Vec<u8>` with full IUPAC alphabets. Creation must
uppercase residues and validate them against the alphabet. The two kinds
share slicing semantics but differ in operations (reverse complement,
translation are nucleotide-only). Higher-level documents
(`SequenceDocument`, `AlignmentDocument`) must contain them.

## Decision

Define `Sequence<A: Alphabet>` as an immutable struct wrapping `Vec<u8>`
with `PhantomData<A>`, where `Alphabet` is a sealed trait implemented by
zero-sized markers (`IupacDna`, `IupacRna`, `IupacAminoAcid`) backed by
256-entry validation tables. Validation happens in a fallible `try_new`
returning the crate's typed `Error`. Alphabet-specific operations live on
concrete instantiations (`impl Sequence<IupacDna>`,
`impl Sequence<IupacRna>`); documents are generic over the
same parameter (`SequenceDocument<A>`, `AlignmentDocument<A>`), giving
compile-time alphabet homogeneity.

## Alternatives Considered

- **Generic struct + marker alphabets** (chosen) — shared code once,
  alphabet errors at compile time; mirrors Geneious's marker-interface
  dispatch, but statically.
- **Trait + one struct per alphabet** — duplicates slicing/storage code and
  cannot share generic document types.
- **Runtime enum `Sequence::Nucleotide(Vec<u8>)`/`AminoAcid`** — alphabet
  becomes a runtime decision; consumers must match everywhere. Acceptable
  only at I/O boundaries before the alphabet is known.
- **Untyped bytes** (IGV/JBrowse style) — insufficient for a
  sequence-editing application.

## Consequences

- ✅ Alphabet misuse is a compile error; shared slicing/validation written
  once; zero-cost markers.
- ❌ Generic documents propagate `<A>` through the model; mixed-type
  collections need partitioning (like Geneious's sequence lists) or a
  boundary enum.

## AI Guidance

When working in this area, an agent should:

- **Preserve:** The invariant that a `Sequence<A>` is always uppercase and
  IUPAC-valid — no public mutation path that bypasses validation.
- **Avoid:** Adding a trait hierarchy for sequences; runtime alphabet
  enums in core APIs.
- **Prefer:** Extending behavior via `impl Sequence<SpecificAlphabet>`
  blocks; copy-on-write transformations returning new values.
- **Ask before changing:** The sealed `Alphabet` trait members and the
  storage type, since both are public API.

## Links

- Supersedes: ~
- Related: `docs/research/geneious/02-sequence-model.md`
