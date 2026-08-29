# Agent Guide for Mab

## Project Overview

Mab (My Alignment Browser) is a free, open-source, cross-platform genome browser
inspired by Geneious Prime. It is a Rust learning and portfolio project built
with deliberate, spec-driven, ADR-documented design.

- **Repository:** https://github.com/jfear/mab
- **License:** MIT
- **Author:** Justin Fear <justin.m.fear@gmail.com>

## Tech Stack

- **Language:** Rust (edition 2024, MSRV 1.85)
- **GUI framework:** Iced (intended)
- **Workspace:** Cargo workspace with two crates:
  - `crates/mab` — application binary
  - `crates/mab-core` — shared domain types and algorithms
- **Error handling:** `anyhow` for application code, `thiserror` for library code
- **Observability:** `tracing` + `tracing-subscriber`
- **Linting:** `clippy::pedantic` and `clippy::nursery` enabled as warnings

## Conventions

- Prefer explicit, readable code over cleverness.
- Keep `mab-core` free of application/framework dependencies; it owns domain
  types and algorithms.
- Document significant architectural choices as ADRs in `docs/decisions/`.
- Update `docs/decisions/README.md` whenever a new ADR is added.
- Consult `docs/geneious_reviews/` when designing data models, storage, or the
  operation pipeline; it is a detailed analysis of Geneious Prime's public API
  (pinned to the 2026.1.2 SDK).
- Consult `docs/igv_reviews/` when designing data models, tracks, or storage;
  it is a detailed analysis of IGV's architecture (pinned to v3.0.0-beta.4).
- Place reusable project skills in `./skills/<skill-name>/SKILL.md`.

## ADR Process

Before making or changing a significant architectural decision, check whether an
ADR exists in `docs/decisions/`. If not, draft one using
`docs/decisions/ADR-0000-template.md`, keep it to 150–250 words, and update the
index. Propose consequential changes to the user instead of silently changing
accepted ADRs.
