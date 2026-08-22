---
id: ADR-0001
title: Target Linux and Windows Desktops via GitHub Releases
status: accepted
date: 2025-08-22
supersedes: ~
superseded-by: ~
tags: [platform, distribution]
---

## Context

Mab is a personal-use genome browser and portfolio project. To ship a usable first version without spreading effort across mobile, web, and packaging ecosystems, we need to narrow the target platform.

## Decision

Mab will target desktop Linux and Windows only, distributed through GitHub Releases and possibly crates.io.

## Alternatives Considered

- **Desktop Linux + Windows only** (chosen) — focused scope; matches the author's machines and likely early users.
- **Linux + Windows + macOS** (rejected) — broader reach but adds code signing, notarization, and hardware access for testing.
- **Web / WASM** (rejected) — would sidestep distribution but conflicts with native desktop learning goals and large-file genomics workloads.
- **Mobile (iOS / Android)** (rejected) — not aligned with a scientific desktop workflow.

## Consequences

- ✅ Focuses testing and CI on two accessible platforms.
- ✅ Simplifies packaging and release automation.
- ✅ Matches the author's daily machines and likely early users.
- ❌ Excludes macOS users and limits portfolio breadth.
- ❌ Requires explicit cross-platform path handling and filesystem assumptions.

## AI Guidance

When working in this area, an agent should:

- **Preserve:** Linux/Windows compatibility; no platform-specific code without `cfg` guards.
- **Avoid:** macOS-only dependencies, web-first assumptions, and mobile UI patterns.
- **Prefer:** crates with Tier 1 Linux/Windows support and permissive licenses.
- **Ask before changing:** Adding macOS support or switching to web/WASM.

## Links

- Supersedes: ~
- Related: ~
