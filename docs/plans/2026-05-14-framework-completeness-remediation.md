# Crablet Framework Completeness Remediation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bring Crablet to a single stable runtime surface where backend APIs, frontend workflows, and agent capabilities are aligned, testable, and documented.

**Architecture:** Consolidate all product traffic through the gateway, retire legacy web drift, complete the skill and memory execution path, and explicitly separate stable runtime modules from experimental agent modules. Each phase ends with compile, API, and UI verification so the framework matches its advertised capabilities.

**Tech Stack:** Rust, Axum, Tokio, SQLx, SQLite, Redis, React 19, TypeScript, Vite, Zustand, SSE, WebSocket

---

## Scope Summary

This plan addresses the biggest completeness gaps found in the current codebase:

1. Two parallel web control planes exist today.
2. Frontend API resolution still contains hard-coded gateway fallbacks.
3. Skill trigger and fusion memory paths are only partially wired.
4. Agent capability surface is wider than the actual stable runtime.
5. Several frontend persistence features are local-only.
6. Multimodal, RPA, and connector features still contain critical placeholders.

## Exit Criteria

- One officially supported web control plane.
- No frontend hard-coded gateway port fallback.
- `cargo check --manifest-path crablet/Cargo.toml --tests` passes.
- `npm run type-check` passes in a reproducible frontend environment.
- Skill trigger supports at least keyword, regex, command, and intent routes in production.
- Fusion memory session flow is actually used at runtime, or explicitly feature-gated and documented.
- Public docs match the stable feature set.

---

### Task 1: Consolidate the Web Control Plane

**Files:**
- Modify: `crablet/src/channels/cli/mod.rs`
- Modify: `crablet/src/channels/cli/handlers/web.rs`
- Modify: `crablet/src/channels/web.rs`
- Modify: `crablet/src/gateway/server.rs`
- Modify: `crablet/src/channels/cli/args.rs`
- Test: `crablet/tests/e2e_full.rs`
- Test: `crablet/tests/gateway_entrypoint_test.rs`
- Docs: `README.md`
- Docs: `docs/getting-started.md`

**Step 1: Write the failing integration test**

Create `crablet/tests/gateway_entrypoint_test.rs` that boots the supported web entrypoint and asserts:

- `/health` responds successfully.
- `/api/v1/chat/stream` is present.
- the unsupported legacy API path is absent unless an explicit compatibility flag is enabled.

Use `tower::ServiceExt` request assertions against the built router instead of manual browser checks.

**Step 2: Run test to verify current drift**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml gateway_entrypoint_test -- --nocapture
```

Expected:

- At least one assertion fails because the supported runtime still depends on multiple startup paths.

**Step 3: Write the minimal implementation**

Implementation checklist:

- Define one official web-serving path in `cli/mod.rs`.
- Make `ServeWeb` a thin wrapper around the gateway, or remove it from the stable surface.
- Restrict `channels/web.rs` to static UI hosting only, or explicitly mark it as compatibility mode.
- Ensure `gateway/server.rs` is the only runtime that exposes the supported API set.
- Update CLI help text to match the supported launch mode.

**Step 4: Run tests and smoke checks**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml gateway_entrypoint_test -- --nocapture
cargo check --manifest-path crablet/Cargo.toml --tests
```

Expected:

- Router test passes.
- Full backend compile still passes.

**Step 5: Commit**

```bash
git add crablet/src/channels/cli/mod.rs crablet/src/channels/cli/handlers/web.rs crablet/src/channels/web.rs crablet/src/gateway/server.rs crablet/src/channels/cli/args.rs crablet/tests/gateway_entrypoint_test.rs crablet/tests/e2e_full.rs README.md docs/getting-started.md
git commit -m "refactor: unify the supported web control plane"
```

---

### Task 2: Remove Frontend Endpoint Drift and Restore Type-Check Validation

**Files:**
- Modify: `frontend/src/hooks/useStreamingChat.ts`
- Modify: `frontend/src/api/client.ts`
- Modify: `frontend/src/services/api.ts`
- Modify: `frontend/src/services/swarmService.ts`
- Modify: `frontend/package.json`
- Modify: `.github/workflows/` relevant frontend CI workflow
- Test: `frontend/src/hooks/__tests__/useApi.test.ts`
- Test: `frontend/src/utils/__tests__/constants.test.ts`
- Docs: `frontend/README.md`

**Step 1: Write the failing frontend tests**

Add tests that assert:

- streaming chat uses configured API base URL resolution.
- no hard-coded `localhost:18789` fallback is used when a valid configured base URL exists.
- swarm service paths resolve through the same API prefix strategy.

**Step 2: Run test and type-check to verify failure**

Run:

```bash
npm run test:ci -- useApi
npm run type-check
```

Expected:

- API-resolution tests fail or expose the hard-coded fallback.
- `type-check` currently cannot be trusted until the frontend environment is reproducible.

**Step 3: Write the minimal implementation**

Implementation checklist:

- Centralize API base URL resolution in one frontend module.
- Make `useStreamingChat.ts` consume that shared resolver instead of maintaining a custom candidate list.
- Normalize `/api/v1/*` path construction across services.
- Update workflow/CI docs so `npm ci` installs TypeScript and `npm run type-check` becomes a required check.

**Step 4: Run verification**

Run:

```bash
npm ci
npm run type-check
npm run test:ci -- useApi
```

Expected:

- TypeScript compiler is available.
- API tests pass.
- No direct gateway-port assumptions remain in production code.

**Step 5: Commit**

```bash
git add frontend/src/hooks/useStreamingChat.ts frontend/src/api/client.ts frontend/src/services/api.ts frontend/src/services/swarmService.ts frontend/package.json frontend/src/hooks/__tests__/useApi.test.ts frontend/src/utils/__tests__/constants.test.ts frontend/README.md .github/workflows
git commit -m "fix: normalize frontend api routing and validation"
```

---

### Task 3: Complete Skill Trigger Routing and Re-enable Fusion Memory Session Flow

**Files:**
- Modify: `crablet/src/cognitive/router.rs`
- Modify: `crablet/src/skills/trigger.rs`
- Modify: `crablet/src/skills/registry.rs`
- Modify: `crablet/src/memory/fusion/mod.rs`
- Modify: `crablet/src/memory/fusion/layer_session.rs`
- Test: `crablet/tests/router_test.rs`
- Test: `crablet/tests/test_skills_agent.rs`
- Test: `crablet/tests/fusion_integration_test.rs`
- Docs: `docs/OPENCLAW_ALIGNMENT.md`

**Step 1: Write the failing tests**

Add coverage for:

- intent-based skill trigger activation from router input.
- router fallback behavior when a skill trigger fails execution.
- fusion session read/write during a normal `process()` request path.

Representative test target:

```rust
#[tokio::test]
async fn router_executes_intent_trigger_before_cognitive_routing() { /* ... */ }
```

**Step 2: Run tests to verify the current gap**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml router_test test_skills_agent fusion_integration_test -- --nocapture
```

Expected:

- intent trigger test fails because `evaluate_intent` is still a placeholder.
- fusion path test fails or proves the session branch is still disabled.

**Step 3: Write the minimal implementation**

Implementation checklist:

- Wire `SkillTrigger::Intent` into the existing classifier.
- Decide whether `SkillTrigger::Semantic` is implemented now or explicitly gated off with a clear runtime error and docs.
- Re-enable fusion session creation and update inside `CognitiveRouter::process`.
- Make router behavior deterministic when both a trigger and a cognitive route are possible.

**Step 4: Run verification**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml router_test -- --nocapture
cargo test --manifest-path crablet/Cargo.toml test_skills_agent -- --nocapture
cargo test --manifest-path crablet/Cargo.toml fusion_integration_test -- --nocapture
cargo check --manifest-path crablet/Cargo.toml --tests
```

Expected:

- Trigger and fusion tests pass.
- Full backend compile still passes.

**Step 5: Commit**

```bash
git add crablet/src/cognitive/router.rs crablet/src/skills/trigger.rs crablet/src/skills/registry.rs crablet/src/memory/fusion/mod.rs crablet/src/memory/fusion/layer_session.rs crablet/tests/router_test.rs crablet/tests/test_skills_agent.rs crablet/tests/fusion_integration_test.rs docs/OPENCLAW_ALIGNMENT.md
git commit -m "feat: complete skill trigger and fusion session routing"
```

---

### Task 4: Narrow the Stable Agent Runtime Surface

**Files:**
- Modify: `crablet/src/agent/mod.rs`
- Modify: `crablet/src/agent/factory.rs`
- Modify: `crablet/src/agent/swarm/mod.rs`
- Modify: `crablet/src/agent/swarm.rs`
- Docs: `docs/feature-implementation-status.md`
- Docs: `docs/agent_optimization_roadmap.md`
- Docs: `docs/architecture.md`

**Step 1: Write the failing runtime-surface test**

Add or extend a backend test that asserts the supported roles and runtime-exported agent modules are the ones actually constructible through the factory and reachable through the swarm runtime.

**Step 2: Run verification to expose mismatch**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml framework_verify -- --nocapture
```

Expected:

- Existing or new assertions reveal that some documented modules are not part of the stable runtime surface.

**Step 3: Write the minimal implementation**

Implementation checklist:

- Keep stable exports in `agent/mod.rs` aligned with actual runtime usage.
- Mark experimental modules clearly in docs and keep them outside the stable claim set unless they are wired into production.
- Ensure `AgentFactory` and swarm orchestration only advertise roles that are actually supported.

**Step 4: Run verification**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml framework_verify -- --nocapture
cargo check --manifest-path crablet/Cargo.toml --tests
```

Expected:

- Stable runtime surface is explicit.
- Docs and code no longer disagree on what is supported today.

**Step 5: Commit**

```bash
git add crablet/src/agent/mod.rs crablet/src/agent/factory.rs crablet/src/agent/swarm/mod.rs crablet/src/agent/swarm.rs docs/feature-implementation-status.md docs/agent_optimization_roadmap.md docs/architecture.md
git commit -m "docs: align stable agent runtime surface with implementation"
```

---

### Task 5: Persist Canvas and Token State Through the Backend

**Files:**
- Modify: `frontend/src/store/canvasVersionStore.ts`
- Modify: `frontend/src/store/tokenStatsStore.ts`
- Modify: `frontend/src/services/api.ts`
- Modify: `frontend/src/services/workflowApi.ts`
- Modify: `crablet/src/gateway/server.rs`
- Modify: `crablet/src/gateway/web_handlers.rs`
- Modify: `crablet/src/storage/canvas_state.rs`
- Modify: `crablet/src/storage/mod.rs`
- Test: `frontend/src/store/__tests__/tokenStatsStore.test.ts`
- Test: `crablet/tests/integration_test.rs`

**Step 1: Write the failing tests**

Add coverage for:

- canvas version save/load roundtrip.
- token usage sync endpoint persistence.
- rollback event emission contract.

**Step 2: Run tests to verify current local-only behavior**

Run:

```bash
npm run test:ci -- tokenStatsStore
cargo test --manifest-path crablet/Cargo.toml integration_test -- --nocapture
```

Expected:

- frontend store test exposes missing backend sync behavior.
- backend integration test fails because the persistence endpoints are absent or incomplete.

**Step 3: Write the minimal implementation**

Implementation checklist:

- Add gateway endpoints for canvas version persistence and token usage sync.
- Persist canvas versions through storage instead of local-only memory.
- Emit rollback or audit events through the existing event bus contract.
- Update frontend stores to call the backend instead of leaving TODOs.

**Step 4: Run verification**

Run:

```bash
npm run test:ci -- tokenStatsStore
cargo test --manifest-path crablet/Cargo.toml integration_test -- --nocapture
cargo check --manifest-path crablet/Cargo.toml --tests
```

Expected:

- Store behavior is backed by server persistence.
- Backend compile and integration tests pass.

**Step 5: Commit**

```bash
git add frontend/src/store/canvasVersionStore.ts frontend/src/store/tokenStatsStore.ts frontend/src/services/api.ts frontend/src/services/workflowApi.ts crablet/src/gateway/server.rs crablet/src/gateway/web_handlers.rs crablet/src/storage/canvas_state.rs crablet/src/storage/mod.rs frontend/src/store/__tests__/tokenStatsStore.test.ts crablet/tests/integration_test.rs
git commit -m "feat: persist canvas history and token usage"
```

---

### Task 6: Resolve Placeholder Feature Paths in Multimodal, RPA, and Connectors

**Files:**
- Modify: `crablet/src/knowledge/multimodal.rs`
- Modify: `crablet/src/rpa/workflow.rs`
- Modify: `crablet/src/channels/domestic/feishu.rs`
- Modify: `crablet/src/skills/executor.rs`
- Docs: `docs/feature-implementation-status.md`
- Docs: `docs/roadmap.md`
- Test: `crablet/tests/rpa_desktop_test.rs`
- Test: `crablet/tests/vector_store_integration_test.rs`

**Step 1: Write the failing tests**

Add targeted tests for:

- image/audio/video ingestion output never returning placeholder markers in the stable path.
- workflow condition and loop execution traversing nested steps.
- Feishu startup path explicitly reporting unsupported subscription mode until implemented.
- strong-isolation skill execution returning a deterministic capability error if Wasm remains unavailable.

**Step 2: Run tests to capture the current state**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml rpa_desktop_test vector_store_integration_test -- --nocapture
```

Expected:

- Tests fail or expose placeholder text / missing branch execution.

**Step 3: Write the minimal implementation**

Implementation checklist:

- Replace placeholder multimodal strings with minimum viable structured extraction output.
- Implement recursive branch execution for condition and loop workflow nodes.
- Decide whether Feishu event subscription is implemented now or explicitly hidden behind a capability flag.
- Decide whether Wasm isolation is implemented now or demoted from the stable claim set.

**Step 4: Run verification**

Run:

```bash
cargo test --manifest-path crablet/Cargo.toml rpa_desktop_test -- --nocapture
cargo test --manifest-path crablet/Cargo.toml vector_store_integration_test -- --nocapture
cargo check --manifest-path crablet/Cargo.toml --tests
```

Expected:

- Stable feature paths no longer rely on placeholder strings or silent TODO branches.

**Step 5: Commit**

```bash
git add crablet/src/knowledge/multimodal.rs crablet/src/rpa/workflow.rs crablet/src/channels/domestic/feishu.rs crablet/src/skills/executor.rs docs/feature-implementation-status.md docs/roadmap.md crablet/tests/rpa_desktop_test.rs crablet/tests/vector_store_integration_test.rs
git commit -m "feat: replace placeholder feature paths with stable behavior"
```

---

## Recommended Commit Order

1. `refactor: unify the supported web control plane`
2. `fix: normalize frontend api routing and validation`
3. `feat: complete skill trigger and fusion session routing`
4. `docs: align stable agent runtime surface with implementation`
5. `feat: persist canvas history and token usage`
6. `feat: replace placeholder feature paths with stable behavior`

## Risks and Mitigations

- **Risk:** Web entrypoint consolidation breaks local development assumptions.
  - **Mitigation:** Add router-level integration tests before removing any path.

- **Risk:** Fusion memory re-enable introduces runtime regressions.
  - **Mitigation:** Start with session-layer-only integration and keep feature gating until tests pass.

- **Risk:** Frontend API normalization breaks custom deployments.
  - **Mitigation:** Keep a single resolver module with explicit override docs and test coverage.

- **Risk:** Experimental agent modules are still useful internally.
  - **Mitigation:** Reclassify them as experimental rather than deleting them.

## Definition of Done

- One supported gateway path for product traffic.
- One frontend API routing strategy.
- Verified backend compile and focused tests for each phase.
- Verified frontend type-check and targeted frontend tests.
- Stable feature list matches docs.

Plan complete and saved to `docs/plans/2026-05-14-framework-completeness-remediation.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration

2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints
