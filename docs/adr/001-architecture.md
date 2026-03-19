# ADR 001 — Architecture & Core Design

**Status**: Accepted
**Date**: 2026-03-18

## Context

Sutra is an infrastructure orchestration tool for AGNOS. It needs to execute tasks on local and remote nodes, support multiple transport mechanisms, and be extensible with new modules.

Key constraints:
- Rust async ecosystem — `dyn Trait` and `async fn` are incompatible without boxing
- Modules must work across local, SSH, and daimon transports without code changes
- Infrastructure tooling must be safe by default (dry-run, idempotency, audit)

## Decision

### Executor with enum dispatch

Instead of `dyn Transport` (which conflicts with async), use a concrete `Executor` struct with internal `ExecutorKind` enum. Modules receive `&Executor` and call its methods directly.

```
ExecutorKind::Local  — tokio::process::Command + tokio::fs
ExecutorKind::Ssh    — russh sessions, lazy connect
```

This avoids `async_trait` boxing overhead and keeps the API simple.

### Module trait with enum dispatch

`SutraModule` is a trait with `async fn` methods (using `#[allow(async_fn_in_trait)]`). Concrete modules are wrapped in a `Module` enum in `sutra-modules` for dispatch. This avoids `dyn SutraModule` entirely.

### Shell execution model

All module operations reduce to shell commands executed via `Executor::exec()`. Parameters are escaped with `shlex::try_quote()` before interpolation. This keeps modules simple and portable across transports.

### Playbook-first design

TOML is the canonical format. YAML and Markdown are convenience inputs that convert to TOML. Natural language goes through hoosh to produce TOML. The user always reviews TOML before execution.

## Consequences

- Adding a new transport = adding a variant to `ExecutorKind` + implementing 4 methods
- Adding a new module = implementing `SutraModule` trait + adding to `Module` enum + registering in `ModuleRegistry`
- No dynamic plugin loading (compile-time registration only) — community modules ship as separate crates
- Shell escaping is a security-critical path — all user params must go through `esc()`
