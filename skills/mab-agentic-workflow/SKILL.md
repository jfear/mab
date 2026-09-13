---
name: mab-agentic-workflow
description: Use when starting, planning, implementing, reviewing, or closing a non-trivial OpenSpec change in the Mab repository.
---

# Mab Agentic Workflow

**REQUIRED BASELINE:** Use `using-openspec-superpowers` before applying this adapter.

## Mab constraints

- Use the committed `agentic-workflow` OpenSpec schema and read every active change artifact before implementation.
- Work in a Worktrunk feature worktree. Use `wt`; do not use raw `git worktree`.
- Keep `.agent-work/` ignored and per-worktree. Never stage it.
- Treat `openspec/specs/` as current behavior. Retain active-change `plan.md` and `verify.md`; archive them only after shipment.
- Use `docs/decisions/` only for consequential architectural decisions. **REQUIRED SUB-SKILL:** use `adr` when an ADR is needed.
- For data models, storage, operations, tracks, or UI/session decisions, consult the applicable research under `docs/research/{geneious,igv,jbrowse}/` before proposing a design.

## Verification

For Rust changes, run the focused test first, then `cargo test --workspace`, `cargo clippy --workspace --all-targets`, and `cargo fmt --check` before final review. Run `dprint fmt` for Markdown, TOML, YAML, JSON, and other supported documentation/configuration changes. Record executed commands and results in the schema-managed `verify.md`.

## Closeout

Before final approval, reconcile artifacts, sync the delta specs, and verify no `.agent-work/` material is staged. Do not push, merge, archive, or remove a worktree without the owner’s explicit approval.
