## Why

Mab needs a repeatable, reviewable agentic development workflow that keeps OpenSpec as the committed source of product behavior, retains one finalized implementation plan with each change's history, and prevents transient agent material from becoming stale repository documentation.

## What Changes

- Define the durable ownership and lifecycle of OpenSpec artifacts, archived implementation plans and verification records, ADRs, code, and ephemeral `.agent-work` material.
- Configure ignored per-worktree `.agent-work` directories through Worktrunk, without sharing state between worktrees.
- Add a minimal custom OpenSpec schema, a version-controlled Mab workflow adapter skill, and a separately versioned global skill repository for reusable workflow discipline.
- Define branch, commit, specification-sync, review, archive, and cleanup checkpoints.

## Capabilities

### New Capabilities

- `agentic-development-workflow`: Governs how agents create, execute, review, synchronize, archive, and clean up spec-driven changes in Mab.

### Modified Capabilities

- None.

## Impact

- Adds repository workflow configuration, ignore rules, and a project skill.
- Creates a personal global-skills repository outside Mab and registers it with Pi.
- Changes contributor/agent process but does not affect Mab runtime behavior, public APIs, or dependencies.
