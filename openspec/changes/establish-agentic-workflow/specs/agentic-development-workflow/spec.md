## Purpose

Defines a repeatable, spec-driven agent workflow that keeps Mab's behavioral documentation current and archives its implementation plans and verification evidence with shipped changes.

## ADDED Requirements

### Requirement: Durable artifact ownership

The workflow SHALL treat `openspec/specs/` as the committed source of truth for current product behavior. Active OpenSpec changes SHALL retain their proposal, delta specifications, design, coarse tasks, reviewed detailed implementation plan, and final verification record until archival. `docs/decisions/` SHALL contain only consequential architectural decisions and their rationale. Agent research, plan drafts, raw review packets, prompts, and logs SHALL be stored under ignored `.agent-work/` directories unless deliberately promoted to a durable document.

#### Scenario: An agent finalizes an execution plan

- **WHEN** a reviewed OpenSpec change is prepared for implementation
- **THEN** its required `plan.md` artifact contains the detailed Superpowers implementation plan and is retained with that change through archival

#### Scenario: An agent verifies implementation

- **WHEN** implementation is ready for final review
- **THEN** its required `verify.md` artifact records the specification, test, and review evidence used to assess the change before archival

#### Scenario: An agent drafts an execution plan

- **WHEN** an agent is developing an implementation plan before it is reviewed
- **THEN** the draft is stored in `.agent-work/plans/` and is not staged for commit

#### Scenario: A design decision becomes consequential

- **WHEN** implementation establishes an architectural decision that affects future project direction
- **THEN** the decision is recorded in `docs/decisions/` rather than retained only in agent working material

### Requirement: Per-worktree agent workspace isolation

Every Mab checkout used for agent work SHALL provide `.agent-work/research`, `.agent-work/reviews`, `.agent-work/plans`, `.agent-work/specs`, and `.agent-work/prompts`. The workspace SHALL be ignored by Git and SHALL be a real directory within its own worktree; agent workspaces from different worktrees SHALL NOT share mutable state.

#### Scenario: Worktrunk creates a feature worktree

- **WHEN** Worktrunk creates a Mab feature worktree
- **THEN** its blocking startup lifecycle creates the standard `.agent-work` directory layout before work begins

#### Scenario: A feature worktree is removed

- **WHEN** Worktrunk removes a feature worktree
- **THEN** the worktree's `.agent-work` contents are removed with that worktree without affecting another checkout's agent workspace

### Requirement: Spec-driven change lifecycle

For a non-trivial Mab change, exploration SHALL precede a named OpenSpec change when requirements are uncertain. A feature branch and worktree SHALL be created before the OpenSpec proposal is authored. The Mab OpenSpec schema SHALL require the ordered planning artifacts `proposal.md`, delta specifications, `design.md`, `tasks.md`, and `plan.md`, followed by required `verify.md` evidence before archival. Implementation SHALL begin only after the planning artifacts are reviewed. Material discoveries during implementation SHALL reconcile the active OpenSpec artifacts before final review.

#### Scenario: Exploration produces a buildable change

- **WHEN** exploration has established a proposed change's scope
- **THEN** the workflow creates a feature worktree before creating the named OpenSpec change and its proposal artifacts

#### Scenario: Implementation changes a requirement

- **WHEN** implementation reveals that an accepted requirement or design no longer reflects the intended behavior
- **THEN** the active OpenSpec artifacts are updated and reviewed before implementation is considered complete

### Requirement: Synchronization and closeout

Before final approval of an implementation pull request, the workflow SHALL validate the implementation against the active OpenSpec change, complete `verify.md`, and sync its delta specifications into `openspec/specs/`. After the implementation is merged, the workflow SHALL archive the shipped OpenSpec change, including its finalized `plan.md` and `verify.md`, from the main worktree. Ephemeral agent material SHALL NOT be committed and the feature worktree SHALL be removed only after its branch is integrated or the owner explicitly preserves it.

#### Scenario: A change is ready for final pull-request review

- **WHEN** implementation and agent review are complete
- **THEN** the delta specifications are synced into `openspec/specs/` and the final review evaluates the proposal, specifications, and code together

#### Scenario: A shipped change is closed

- **WHEN** the implementation pull request has merged
- **THEN** the change is archived from the main worktree and the associated feature worktree may be removed

### Requirement: Reusable workflow skills

The generic agentic workflow and its minimal OpenSpec schema SHALL be maintained in a separately version-controlled global skill repository and loaded by Pi from its configured skill path. Mab SHALL install the schema and maintain a committed project adapter skill for its Rust verification, ADR, and domain-research conventions.

#### Scenario: An agent starts work in Mab

- **WHEN** an agent invokes the global workflow skill in the Mab repository
- **THEN** the committed Mab adapter supplies project-specific constraints without duplicating the generic workflow skill
