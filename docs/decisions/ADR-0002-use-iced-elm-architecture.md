---
id: ADR-0002
title: Use Iced with Elm-Like Architecture for the GUI
status: accepted
date: 2025-08-22
supersedes: ~
superseded-by: ~
tags: [gui, framework, architecture]
---

## Context

Mab needs a native, cross-platform GUI in Rust that can render interactive genomic alignments. The framework should fit Rust's ownership model, be composable, and support a deliberate, testable UI structure.

## Decision

Use the Iced framework, adopting its Elm-like architecture (Model-Update-View with subscriptions and commands) for the main application.

## Alternatives Considered

- **Iced with Elm-like architecture** (chosen) — native, composable, and idiomatic for a testable Rust desktop app.
- **egui** (rejected) — immediate-mode and excellent for tools, but less suited to a traditional multi-view desktop application structure.
- **Tauri** (rejected) — web-based stack would add JavaScript complexity and undermine the native Rust learning goal.
- **Slint** (rejected) — declarative DSL is less idiomatic for fine-grained Rust control and custom rendering.

## Consequences

- ✅ Strong type safety and composable widgets.
- ✅ Reactive, testable update loop separates state from effects.
- ✅ Native feel on Linux and Windows without web dependencies.
- ❌ Smaller ecosystem and fewer ready-made components than Electron/web stacks.
- ❌ Custom visualization widgets (e.g., alignment tracks) require deeper Iced knowledge.

## AI Guidance

When working in this area, an agent should:

- **Preserve:** The Elm-like split of Model, Message, Update, and View.
- **Avoid:** Immediate-mode patterns, web-tech stacks, and mixing view logic with side effects.
- **Prefer:** Iced built-in widgets and subscriptions before writing custom ones.
- **Ask before changing:** Renderer backend, custom rendering pipeline, or switching away from Iced.

## Links

- Supersedes: ~
- Related: [ADR-0001](ADR-0001-desktop-platforms.md)
