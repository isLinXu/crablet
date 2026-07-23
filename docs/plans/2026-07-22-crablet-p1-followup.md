# Crablet P1 Follow-up Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the remaining P1 contract gaps in provider selection and runtime event replay without breaking legacy configuration or tool-calling behavior.

**Architecture:** Keep capability routing as a pure deterministic selector, then use its ordered candidates to construct the existing fallback client. Preserve the legacy single-model/Ollama path when no provider registry is configured. Persist the complete runtime envelope in SQLite and make replay return the stored identity, including compatibility with pre-migration rows.

**Tech Stack:** Rust 2021, Tokio, SQLx SQLite, Serde, existing `LlmClient` and `FallbackLlmClient` abstractions.

---

### Task 1: Stabilize provider selection compatibility

**Files:**
- Modify: `crablet/src/cognitive/mod.rs`
- Test: `crablet/src/cognitive/mod.rs`

Preserve `Config::ollama_model` for the legacy Ollama configuration, retain TOML model order when no explicit fallback order exists, and keep provider iteration deterministic.

### Task 2: Make runtime replay identity durable

**Files:**
- Modify: `crablet/src/events.rs`
- Modify: `crablet/migrations/20260722000000_runtime_event_envelope.sql`
- Test: `crablet/src/events.rs`

Add SQLite-backed regression coverage for event identity and correlation coordinates, and ensure replay remains compatible with rows written before the migration.

### Task 3: Integrate ordered provider candidates with runtime fallback

**Files:**
- Modify: `crablet/src/cognitive/mod.rs`
- Modify: `crablet/src/cognitive/llm/fallback.rs`
- Test: `crablet/src/cognitive/mod.rs`
- Test: `crablet/src/cognitive/llm/fallback.rs`

Construct the existing fallback client from configured candidates while preserving capability filtering and deterministic order. Keep tool requests on clients that support real tool forwarding instead of silently converting them to plain text.

### Task 4: Verify all supported feature combinations

**Files:**
- No source changes expected.

Run formatting, checks, targeted tests, and the repository quality script. Record environment-only blockers separately from code failures.
