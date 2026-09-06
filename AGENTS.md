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
└── research/    # Background research, comparisons, invariant deep-dives
```

When working in a **git worktree**, `.pi/` is not part of the worktree
because it is gitignored and untracked. The agent may need to read specs
or plans from the **main repository's** `.pi/` directory rather than
expecting them in the worktree. If the workflow creates temporary
scratch files specific to a worktree session, place them in the
worktree's `.pi/` (create it if needed — it will be cleaned up when the
worktree is removed).

Create worktrees under `.pi/worktrees/` to keep them co-located and
easy to find:

```bash
git worktree add .pi/worktrees/feat-adr-NNNN -b feat/adr-NNNN-short-name
```

After the feature branch is merged and the PR is closed, clean up:

```bash
git worktree remove .pi/worktrees/feat-adr-NNNN
git branch -d feat/adr-NNNN-short-name
```

Then ask the user whether to delete the corresponding spec, plan, and
research files from `.pi/` — they may want to keep them for reference
or clear them out now that the decision has landed in an ADR.

## ADR Process

Before making or changing a significant architectural decision, check whether an
ADR exists in `docs/decisions/`. If not, draft one using
`docs/decisions/ADR-0000-template.md`, keep it to 150–250 words, and update the
index. Propose consequential changes to the user instead of silently changing
accepted ADRs.
