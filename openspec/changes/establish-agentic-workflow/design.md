## Context

See `proposal.md` for motivation. Mab has an OpenSpec `spec-driven` setup, globally installed Superpowers skills, Worktrunk v0.77.0, and an ignored `.pi/` scratch area. It currently has no committed workflow capability, ignored `.agent-work` directory, Worktrunk project configuration, or project adapter skill.

## Goals / Non-Goals

**Goals:**

- Make OpenSpec specifications the durable description of current behavior.
- Retain a reviewed implementation plan and final verification record with each active OpenSpec change and archive them with the change.
- Keep drafts and raw agent execution material isolated and disposable.
- Make the process reusable across projects while retaining Mab-specific policy.
- Make worktree creation and cleanup safe by default.

**Non-Goals:**

- Adopt or vendor Superspec.
- Vendor Superspec or retain its broader apply/review/finalization receipt set.
- Automate pull-request creation, merging, or archival without an explicit user decision.
- Share live agent work areas between worktrees.

## Decisions

### Own a minimal OpenSpec/Superpowers schema

Mab installs an owned OpenSpec schema that extends the normal change graph with `plan.md` and `verify.md` artifacts. The schema makes those files visible to OpenSpec status, instructions, and validation rather than relying on undocumented extra files. Its ordered lifecycle is proposal → specs and design → tasks → plan → implementation → verify → sync/archive.

OpenSpec does not enforce artifact completion at archive time. Therefore the global workflow closeout is the archive guard: it SHALL inspect the active change, require `verify.md`, and require its READY verdict before it offers archive to the owner. OpenSpec owns the durable proposal, delta specification, design, task, plan, verification, sync, and archive lifecycle. Superpowers creates the detailed plan and implementation evidence through schema instructions. `.agent-work/` holds only drafts and raw material before it is distilled into the durable artifacts.

Superspec was considered because it connects these systems. Mab adopts its useful artifact lifecycle without taking a dependency or retaining its broader apply, review, and finalization receipts.

### Use a tracked global-skill source and a committed Mab adapter

A new `~/Projects/agent-skills` Git repository owns generic skills. Pi loads its `skills/` directory through `~/.pi/agent/settings.json`. Mab owns a thin adapter in `skills/mab-agentic-workflow/SKILL.md`; it references generic skills and adds Rust validation, ADR, and research conventions.

This separates reusable process from project policy while permitting both to evolve under version control.

### Use per-worktree real directories

`.agent-work/` is ignored in Mab and each worktree creates the standard leaf directories. A committed `.config/wt.toml` uses Worktrunk's blocking `pre-start` hook so a new checkout is ready before an agent begins. The existing main worktree is initialized idempotently by the workflow skill.

A shared symlink was rejected because concurrent worktrees would share mutable notes, locks, and path-sensitive state. Copying live agent state was rejected because it creates stale and ambiguous ownership.

### Branch before proposal; archive after shipment

Read-only exploration occurs in the current checkout. Once a change is named, Worktrunk creates `feature/<change>` and OpenSpec planning occurs there. The same feature worktree contains implementation commits. Before final approval, the change is validated and synced so current canonical specs land with the code. After merge, archive runs from main, preserving history only for shipped behavior.

## Risks / Trade-offs

- [Global skill repository unavailable] → The Mab adapter names the required generic skill and fails clearly; Pi's configured path can be restored by cloning the repository.
- [Worktrunk hooks are bypassed] → The global workflow skill initializes `.agent-work` idempotently and treats a missing directory as a preflight failure.
- [Implementation drifts from specs] → Require reconciliation and OpenSpec sync before final review.
- [Extra documentation burden] → Retain OpenSpec artifacts, one finalized change-local plan, one verification record, and ADRs; drafts and raw agent material remain ignored and are deleted with the worktree.

## Migration Plan

1. Create the minimal custom schema with required `plan.md` and `verify.md` artifacts, and configure Mab to use it.
2. Add the ignore rule, Worktrunk hook configuration, and Mab adapter skill.
3. Create and version the global skill repository; configure Pi to load it.
4. Have the global skill create the schema artifacts through OpenSpec instructions.
5. Initialize `.agent-work` in the existing main checkout.
6. Validate skill discovery, schema artifact creation, and Worktrunk startup in a disposable worktree.
7. Use this change as the first lifecycle trial, then revise the adapter if review reveals gaps.
