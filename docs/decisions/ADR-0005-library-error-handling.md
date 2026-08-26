---
id: ADR-0005
title: Use thiserror for Library Crates and anyhow for Binaries
status: accepted
date: 2026-08-26
supersedes: ~
superseded-by: ~
tags: [error-handling, api-design, dependencies]
---

## Context

Mab is a workspace of library crates (`mab-core`) and an application binary
(`mab`). Library callers need typed, inspectable errors; the binary needs
ergonomic error propagation and end-user reporting.

## Decision

Library crates use `thiserror`; binary crates use `anyhow`. Each fallible
library crate gets an `error.rs` defining `pub enum Error` with
`#[derive(Debug, thiserror::Error)]` and `#[non_exhaustive]`, plus a
`pub type Result<T> = std::result::Result<T, Error>;` alias re-exported from
`lib.rs` and used throughout the crate. External errors are converted at the
boundary via `#[from]` or wrapped with context via `#[source]`. Infallible
crates skip this entirely.

## Alternatives Considered

- **`thiserror` + `anyhow`** (chosen) — the 2026 community standard; actively
  maintained; MSRV-compatible (thiserror 2.x needs Rust 1.61).
- **`snafu`** — solid but smaller adoption and a different idiom.
- **`error-stack`** — pre-1.0 and niche.
- **`miette`** — rich CLI diagnostics, not a library error type.
- **`Box<dyn Error>` / string errors** — loses typing and source chains.

## Consequences

- ✅ Typed errors with preserved source chains; `#[non_exhaustive]` allows
  non-breaking variant additions; binaries convert via `?` for free.
- ❌ Per-crate boilerplate and care needed when choosing `#[from]` boundaries.

## AI Guidance

When working in this area, an agent should:

- **Preserve:** `error.rs` per fallible library crate; the crate-local
  `Result` alias; named error type `Error` (not crate-prefixed).
- **Avoid:** Leaking external error types past boundaries; `Box<dyn Error>`
  in library APIs; blanket `#[from]` for every dependency error; flattening
  one crate's error variants into another crate's enum; stringifying an error
  across an internal boundary (`Display` and the source chain must survive).
- **Prefer:** Context-bearing variants (`#[source]` + descriptive name) when
  the failure site adds useful information. Wrapping a sibling crate's whole
  error as one `#[error(transparent)] #[from]` variant in the downstream
  crate's enum; add a context-bearing variant only when the downstream crate
  has operation-specific information to attach.
- **Ask before changing:** The error layout of a published crate, since it is
  public API.

## Links

- Supersedes: ~
- Related: ~
