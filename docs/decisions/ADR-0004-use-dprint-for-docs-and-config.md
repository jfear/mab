---
id: ADR-0004
title: Use dprint for Markdown and Configuration Formatting
status: accepted
date: 2025-08-22
supersedes: ~
superseded-by: ~
tags: [tooling, formatting, docs]
---

## Context

As documentation and configuration files grow, we need consistent formatting without relying on manual edits. A single formatter for Markdown and common config files keeps the repository tidy and review diffs focused.

## Decision

Use [dprint](https://dprint.dev/) to format Markdown, TOML, and JSON files in the repository.

## Alternatives Considered

- **dprint** (chosen) — fast, Rust-native, with plugins covering Markdown, TOML, and JSON.
- **Prettier** (rejected) — popular and configurable, but requires Node.js and is slower than dprint.
- **mdformat** (rejected) — Markdown-only and Python-based; would not handle TOML/JSON.
- **Manual formatting** (rejected) — unreliable and produces noisy diffs.

## Consequences

- ✅ Fast, Rust-native formatter with plugins for Markdown, TOML, and JSON.
- ✅ Consistent formatting across READMEs, ADRs, and Cargo manifests.
- ✅ `dprint check` can be added to CI if we want enforcement later.
- ❌ Contributors must install another tool to format files locally.
- ❌ The `dprint.json` plugin versions need occasional updates.

## AI Guidance

When working in this area, an agent should:

- **Preserve:** The `dprint.json` configuration and its file associations.
- **Avoid:** Mixing dprint-formatted files with manual formatting in the same PR.
- **Prefer:** `just fmt-md` and `dprint fmt` over ad-hoc reformatting.
- **Ask before changing:** The formatter choice, `dprint.json` settings, or adding new file types.

## Links

- Supersedes: ~
- Related: [ADR-0003](ADR-0003-build-tooling.md)
