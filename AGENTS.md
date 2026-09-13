# Agent Guide for Mab

## Project Overview

Mab (My Alignment Browser) is a free, open-source, cross-platform genome browser
inspired by Geneious Prime. It is a Rust learning and portfolio project built
with deliberate, spec-driven, ADR-documented design.

- **Repository:** https://github.com/jfear/mab
- **License:** MIT
- **Author:** Justin Fear

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
- The global `rust-style-guide` skill is the idiom baseline; this project's
  conventions (clippy::pedantic/nursery, error-handling split, crate
  boundaries) take precedence where they differ. If Mab's established
  patterns diverge from the guide, Mab wins — flag the divergence, don't
  relitigate it in review.
- Keep `mab-core` free of application/framework dependencies; it owns domain
  types and algorithms.
- Document significant architectural choices as ADRs in `docs/decisions/`.
- Update `docs/decisions/README.md` whenever a new ADR is added.
- Consult `docs/research/geneious/` when designing data models, storage, or the
  operation pipeline; it is a detailed analysis of Geneious Prime's public API
  (pinned to the 2026.1.2 SDK).
- Consult `docs/research/igv/` when designing data models, tracks, or storage;
  it is a detailed analysis of IGV's architecture (pinned to v3.0.0-beta.4).
- Consult `docs/research/jbrowse/` when designing data models, configuration,
  adapters, tracks/displays, or session state; it is a detailed analysis of
  JBrowse 2's architecture (pinned to v4.3.0, commit 83ac4507cf).
- Place reusable project skills in `./skills/<skill-name>/SKILL.md`.
- Use `./.pi/` as a local scratch pad and research area — it is gitignored
  and not part of the published repo. Drop in exploratory notes, working
  drafts, comparison documents, or anything else that helps a design
  discussion without needing to land an ADR first. Anything that survives
  the discussion and becomes a committed decision should graduate into
  `docs/decisions/` (or wherever it belongs); ephemeral material can stay
  here indefinitely.

## Spec-First Development

Mab follows a spec-first workflow: before implementing a feature, write
the spec, plan, and any supporting research documents. These live in
`./.pi/` and are **never committed** — they are ephemeral working
documents used only during development.

```
.pi/
├── specs/       # Implementation specs (self-contained API + behavior)
├── plans/       # Phase-by-phase implementation plans
├── research/    # Background research, comparisons, invariant deep-dives
└── sdd/         # Per-plan subagent-driven-development scratch: task briefs,
                # implementer reports, review packages, progress ledger.
                # Auto-managed and self-gitignored; safe to delete once the
                # plan's work is merged (git history is the record).
```

Worktree setup, teardown, artifact cleanup, and the `.pi/`-in-a-worktree
rules are handled by the global superpowers skills (`using-git-worktrees`,
`finishing-a-development-branch`, `subagent-driven-development`,
`cleanup-dev-artifacts`) — no project-specific overrides are needed.

## ADR Process

Before making or changing a significant architectural decision, check whether an
ADR exists in `docs/decisions/`. If not, draft one using
`docs/decisions/ADR-0000-template.md`, keep it to 150–250 words, and update the
index. Propose consequential changes to the user instead of silently changing
accepted ADRs.
