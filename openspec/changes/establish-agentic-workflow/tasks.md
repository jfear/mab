## 1. OpenSpec workflow schema

- [x] 1.1 Create the minimal owned OpenSpec schema with required `plan.md` and `verify.md` artifacts, then verify OpenSpec reports their dependency order and instructions.
- [x] 1.2 Configure Mab to use the owned schema and verify a disposable change exposes the complete proposal-to-verify artifact lifecycle.

## 2. Worktree workspace foundation

- [x] 2.1 Add the ignored `.agent-work/` policy and committed Worktrunk startup configuration, then verify `.agent-work` is ignored and Worktrunk renders the hook configuration.
- [x] 2.2 Initialize the standard `.agent-work` directory layout in the existing main checkout and verify every required leaf directory exists.

## 3. Global workflow skill source

- [x] 3.1 Create and initialize the version-controlled `~/Projects/agent-skills` repository with the source of its generic workflow skill and minimal OpenSpec schema, then verify the skill has valid Agent Skills frontmatter, Git tracks both sources, and the schema creates the managed plan and verification artifacts.
- [ ] 3.2 Configure Pi to load the global skill repository, then verify a fresh Pi skill discovery lists the generic workflow skill.
- [x] 3.3 Test the generic workflow skill against representative pressure scenarios and refine it until it keeps canonical specs, the archived change-local plan, and the verification record durable while retaining drafts and raw agent material in `.agent-work/`.

## 4. Mab workflow adapter

- [x] 4.1 Create the committed Mab adapter skill that references the generic workflow and supplies Mab-specific ADR, Rust verification, research, and closeout constraints; verify its frontmatter and referenced paths.
- [x] 4.2 Test the adapter against a Mab change-start, plan, verification, and change-close scenario, then verify it selects the required generic workflow and project-specific checks.

## 5. Integration validation

- [ ] 5.1 Create and remove a disposable Worktrunk worktree, verifying isolated `.agent-work` creation and cleanup without modifying another checkout.
- [ ] 5.2 Review the final diff against the OpenSpec change, run OpenSpec strict validation, verify the finalized plan and verification record are schema-managed change artifacts, and verify no `.agent-work` material is staged.
