# Establish Agentic Workflow Verification

## Specification Coverage

| Requirement                            | Evidence                                                                                                                                                                                                      |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Durable artifact ownership             | `agentic-workflow` declares `plan.md` and `verify.md`; the global skill separates those durable artifacts from ignored `.agent-work/` material.                                                               |
| Per-worktree agent workspace isolation | Worktrunk created a disposable worktree with five real `.agent-work` leaf directories; removing it preserved the primary checkout's workspace.                                                                |
| Spec-driven change lifecycle           | A disposable `agentic-workflow` change exposed proposal, specs, design, tasks, plan, and verify in order.                                                                                                     |
| Synchronization and closeout           | The delta was merged into `openspec/specs/agentic-development-workflow/spec.md`; global and Mab skills require spec sync and a READY verification record before archive is offered.                           |
| Reusable workflow skills               | The generic skill and schema are committed in `~/Projects/agent-skills`; a fresh Pi invocation successfully loaded `/skill:using-openspec-superpowers`; the Mab adapter was exercised with the generic skill. |

## Commands Run

| Command                                                                                | Result                                                                                                                        |
| -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `dprint fmt` and `dprint check`                                                        | Passed for Mab workflow artifacts and configuration.                                                                          |
| `openspec schema validate agentic-workflow`                                            | Passed.                                                                                                                       |
| Disposable `openspec new change ... --schema agentic-workflow`                         | Exposed all six ordered artifacts; disposable change was removed.                                                             |
| `wt hook pre-start --dry-run`                                                          | Rendered the agent workspace initialization command.                                                                          |
| Disposable Worktrunk worktree lifecycle                                                | Created five real workspace directories and removed the disposable worktree without force flags.                              |
| `pi --no-session --no-tools --offline --print "/skill:using-openspec-superpowers ..."` | Loaded the generic skill in a fresh Pi process.                                                                               |
| Generic and Mab adapter pressure scenarios                                             | Confirmed durable/ephemeral separation, change reconciliation, project research, verification, and owner-controlled closeout. |
| `openspec validate --specs`                                                            | Passed for `agentic-development-workflow` (with informational long-requirement suggestions).                                  |
| `openspec validate establish-agentic-workflow --strict`                                | Passed.                                                                                                                       |
| `cargo fmt --check`                                                                    | Passed.                                                                                                                       |
| `cargo test --workspace`                                                               | Passed: 99 unit tests and 13 doctests; application crate has 0 tests.                                                         |
| `cargo clippy --workspace --all-targets`                                               | Passed.                                                                                                                       |
| `git diff --check`                                                                     | Passed.                                                                                                                       |

## Review Evidence

An independent final reviewer initially returned `BLOCK` for a missing verification record, incomplete existing-worktree initialization, absent synced canonical spec, and an inaccurate claim that the schema alone enforces archive readiness.

The feature worktree was initialized, the canonical spec was created, this verification record was added, and the design/specification now state the correct split: the schema tracks `verify.md`; the global closeout skill blocks archive unless it contains a READY verdict. A scoped independent re-review found no issues and returned `Merge verdict: OK`.

## Residual Risks

OpenSpec does not mechanically enforce verification completion at archive time. The global and Mab workflow skills are the current archive guard, so a user can bypass the discipline by invoking the CLI directly. This is accepted for now; a future CI or dedicated pre-archive command could add mechanical enforcement.

## Archive Readiness

READY — implementation, verification, and independent review are complete. Human final approval, merge, archive, push, and feature-worktree removal remain owner-controlled actions.
