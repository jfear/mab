# ADR Process for Mab

## When to Use

Use this skill when creating, updating, or reviewing Architecture Decision
Records (ADRs) for the Mab project.

## Procedure

1. Check `docs/decisions/README.md` for the next available ADR number.
2. Copy `docs/decisions/ADR-0000-template.md` to a new file named
   `ADR-XXXX-short-title.md`.
3. Fill in the front matter and all sections, keeping the body to 150–250 words.
4. Update `docs/decisions/README.md` to include the new ADR in the table.
5. If an ADR supersedes an older one, update both the new and old ADR front
   matter (`supersedes` / `superseded-by`), both Links sections, and flip the old
   ADR's status in the index.
6. Requires [dprint](https://dprint.dev/install/). Run `just fmt-md` to format
   Markdown files.
7. Run `just md-check` to ensure Markdown formatting passes.

## Pitfalls

- Do not create an ADR for trivial or reversible choices.
- Do not let ADRs grow into design documents; keep them concise.
- Do not forget to update the index after adding or changing status.

## Verification

- `docs/decisions/README.md` lists the ADR with correct ID, title, status, date,
  and tags.
- The new ADR follows the template format and links back to related records.
- Workspace diagnostics (`just lint` / `just check`) still pass if code was
  changed as part of the decision.
- `just md-check` passes with no Markdown formatting errors.
