# Establish Agentic Workflow Implementation Plan

> **For agentic workers:** Use the approved execution discipline to implement this plan task by task. Keep raw agent records under `.agent-work/`; this file and `verify.md` are the durable change record.

**Goal:** Establish Mab's reusable OpenSpec/Superpowers workflow with schema-managed planning and verification artifacts, isolated agent workspaces, and global plus project-specific skills.

**Architecture:** Mab commits an `agentic-workflow` OpenSpec schema that extends `spec-driven` with `plan.md` and `verify.md`. A separately versioned global skill repository owns reusable process instructions; Mab's committed adapter supplies local policy. Worktrunk creates a private ignored `.agent-work/` directory in every worktree.

**Spec:** `specs/agentic-development-workflow/spec.md`

## Global Constraints

- `openspec/specs/` is the source of truth for current behavior.
- Active changes retain proposal, delta specs, design, tasks, `plan.md`, and `verify.md`; archive preserves them.
- `.agent-work/` is ignored, per-worktree, and never staged.
- Use `wt`, not raw `git worktree`, for lifecycle operations.
- Keep Mab's Rust/ADR/research rules in the committed adapter, not the generic global skill.
- Do not push, merge, archive, or delete a branch/worktree without explicit owner approval.

## Task 1: Schema bootstrap and migration

**Files:**

- Create: `openspec/schemas/agentic-workflow/schema.yaml`
- Create: `openspec/schemas/agentic-workflow/templates/plan.md`
- Create: `openspec/schemas/agentic-workflow/templates/verify.md`
- Modify: `openspec/config.yaml`
- Modify: `openspec/changes/establish-agentic-workflow/.openspec.yaml`

**Requirements:** Spec-driven lifecycle; durable artifact ownership.

- [x] Fork `spec-driven` as `agentic-workflow`, add ordered `plan` and `verify` artifacts, and make `plan` the apply prerequisite.
- [x] Run `openspec schema validate agentic-workflow`; confirm the schema lists six artifacts.
- [x] Select the new schema in project config and migrate this bootstrap change's metadata.
- [x] Create a disposable change with `--schema agentic-workflow`, verify OpenSpec reports `plan` and `verify`, then remove only that disposable change directory.
- [x] Commit the schema bootstrap after the disposable validation passes.

## Task 2: Per-worktree agent workspace

**Files:**

- Modify: `.gitignore`
- Create: `.config/wt.toml`

**Requirements:** Per-worktree agent workspace isolation.

- [x] Add `.agent-work/` to `.gitignore` and verify `git check-ignore .agent-work` succeeds.
- [x] Add a blocking Worktrunk `pre-start` hook that creates `research`, `reviews`, `plans`, `specs`, and `prompts` below `.agent-work/`.
- [x] Run `wt hook pre-start --dry-run` to verify hook rendering without a lifecycle mutation.
- [x] Commit the workspace configuration with the schema bootstrap or as a separate configuration commit.

## Task 3: Global skill repository and generic workflow skill

**Files:**

- Create: `~/Projects/agent-skills/.gitignore`
- Create: `~/Projects/agent-skills/README.md`
- Create: `~/Projects/agent-skills/skills/using-openspec-superpowers/SKILL.md`
- Create: `~/Projects/agent-skills/skills/using-openspec-superpowers/references/pressure-scenarios.md`
- Create: `~/Projects/agent-skills/openspec/schemas/agentic-workflow/schema.yaml`
- Create: `~/Projects/agent-skills/openspec/schemas/agentic-workflow/templates/*.md`

**Requirements:** Reusable workflow skills; durable artifact ownership; synchronization and closeout.

- [x] Run three representative workflow prompts without the new skill and record baseline failures in the global repository's test notes: an urgent feature, an implementation discovery, and a closeout request.
- [x] Create the global Git repository, generic skill with valid Agent Skills frontmatter, and source copy of the minimal OpenSpec schema.
- [x] Encode the workflow: read-only exploration; Worktrunk branch before proposal; native OpenSpec change with schema-managed plan/verify; `.agent-work` scratch boundary; spec sync before final review; archive after merge; no autonomous push/merge/archive/delete.
- [x] Run the same pressure scenarios with the skill loaded; record outcomes and refine the skill to address demonstrated baseline failures.
- [x] Commit the tested global skill repository locally; do not publish or push it.

## Task 4: Pi global-skill registration

**Files:**

- Modify: `~/.pi/agent/settings.json`

**Requirements:** Reusable workflow skills.

- [x] Read and preserve the existing Pi settings structure.
- [x] Add `/home/jfear/Projects/agent-skills/skills` to its additive `skills` array without removing existing settings.
- [x] Start a fresh Pi discovery context or use an equivalent supported inspection to verify `using-openspec-superpowers` is discoverable.

## Task 5: Mab adapter skill

**Files:**

- Create: `skills/mab-agentic-workflow/SKILL.md`

**Requirements:** Reusable workflow skills; spec-driven lifecycle; synchronization and closeout.

- [x] Write a concise project adapter with valid frontmatter that requires the generic skill and provides Mab-specific ADR, Rust verification, domain-research, and Worktrunk constraints.
- [x] Verify the adapter's referenced generic-skill name and all repository-relative paths.
- [x] Run a Mab change-start and closeout pressure scenario with the adapter; verify it preserves the global workflow while adding project checks.
- [x] Commit the adapter and related Mab workflow configuration.

## Task 6: Lifecycle integration verification

**Files:**

- Modify: `openspec/changes/establish-agentic-workflow/tasks.md`
- Create: `openspec/changes/establish-agentic-workflow/verify.md`

**Requirements:** Per-worktree agent workspace isolation; synchronization and closeout.

- [x] Create a disposable Worktrunk worktree from the feature branch, verify the required `.agent-work/` leaf directories are real directories, then remove that disposable worktree without force flags.
- [x] Confirm the primary checkout's `.agent-work/` remains unaffected by the disposable worktree lifecycle.
- [x] Run `openspec validate establish-agentic-workflow --strict`, `git diff --check`, and the project Rust verification commands applicable to this documentation/configuration change.
- [x] Complete `verify.md` with scenario coverage, commands and results, review conclusions, residual risks, and archive-readiness verdict.
- [x] Mark each completed OpenSpec task in `tasks.md`, commit final reconciliation, and request human review.
