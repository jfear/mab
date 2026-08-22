---
id: ADR-0003
title: Adopt cargo-deny, cargo-audit, cargo-machete, and just for Tooling
status: accepted
date: 2025-08-22
supersedes: ~
superseded-by: ~
tags: [tooling, ci, dependencies]
---

## Context

As the workspace grows, we need automated dependency checks and a simple way to run common tasks.

## Decision

Use `cargo-deny` for license and policy auditing, `cargo-audit` for security advisories, `cargo-machete` for detecting unused dependencies, and `just` as the command runner, while staying with `cargo test` for now (`cargo-nextest` remains an option for later).

## Alternatives Considered

- **This toolchain** (chosen) — lightweight, community-standard coverage of license, security, and dependency checks.
- **Add `cargo-nextest` now** (rejected) — useful, but plain `cargo test` is sufficient while the test suite is small.
- **Use Makefiles** (rejected) — `just` is more ergonomic for Rust projects and avoids Makefile syntax pitfalls.
- **Rely only on Cargo and clippy** (rejected) — would let license, security, and dependency bloat issues go unnoticed.

## Consequences

- ✅ Consistent local and CI commands through a `justfile`.
- ✅ Early detection of security advisories, license conflicts, and unused crates.
- ✅ Lightweight tooling with strong community adoption.
- ❌ New contributors must install several Cargo tools.
- ❌ The `justfile` and deny config need occasional maintenance.

## AI Guidance

When working in this area, an agent should:

- **Preserve:** The `justfile` as the single entry point for common tasks.
- **Avoid:** Adding new build or release tools without recording the decision.
- **Prefer:** `just` recipes over long shell commands documented in READMEs.
- **Ask before changing:** The CI toolchain, replacing `cargo test`, or modifying `deny.toml`.

## Links

- Supersedes: ~
- Related: ~
