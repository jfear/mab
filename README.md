![Mab — My Alignment Browser](assets/banner.png)

# Mab — My Alignment Browser

Mab is a free, open-source, cross-platform genome browser inspired by tools like
Geneious Prime. It is being built in public as a passion project for personal
use, portfolio development, and learning.

## Goals

- **Portfolio & Craft:** Demonstrate thoughtful Rust engineering and agentic,
  spec-driven development workflows.
- **GUI Learning:** Explore native desktop GUI programming with the
  [Iced](https://iced.rs/) Rust framework.
- **Deliberate Design:** Use ADRs (Architecture Decision Records) and spec-driven
  development to keep the project slow, intentional, and well-documented.
- **Useful Software:** Build a practical alignment and genome browser that the
  author actually wants to use.

## Project Status

Mab is in its earliest research and scaffolding phase. The workspace is set up
with two crates:

- `crates/mab-core` — shared library for domain types and algorithms.
- `crates/mab` — the main application binary.

Nothing is functional yet; this is a sandbox for learning and exploration.

## Development

This project uses a [Cargo workspace](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
and a [`justfile`](justfile) for common tasks. After installing
[`just`](https://github.com/casey/just),
[dprint](https://dprint.dev/install/) (per
[ADR-0004](docs/decisions/ADR-0004-use-dprint-for-docs-and-config.md)), and the
Cargo tools listed in
[ADR-0003](docs/decisions/ADR-0003-build-tooling.md), run:

```bash
just --list      # see available recipes
just check       # run the full local CI suite
```

Major architectural decisions are recorded as ADRs in
[`docs/decisions/`](docs/decisions/).

## License

This project is licensed under the [MIT License](LICENSE).
