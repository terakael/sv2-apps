# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-05)

**Core value:** Sub-100ms coinbase switching enables fair time-shared mining by allowing rapid address rotation while miners continuously work, without waiting for Bitcoin Core template updates.
**Current focus:** Phase 2 - Share Attribution and Webhooks

## Current Position

Phase: 2 of 3 (Share Attribution and Webhooks)
Plan: 2 of 3 in current phase
Status: In progress
Last activity: 2026-02-05 — Completed 02-02-PLAN.md

Progress: [█████████░] 83% (5/6 plans complete across phases 1-2)

## Performance Metrics

**Velocity:**
- Total plans completed: 5
- Average duration: 9.2 min
- Total execution time: 0.77 hours

**By Phase:**

| Phase | Plans | Total  | Avg/Plan |
|-------|-------|--------|----------|
| 01    | 3     | 35min  | 11.7min  |
| 02    | 2     | 9min   | 4.5min   |

**Recent Trend:**
- Last 5 plans: 01-02 (12min est), 01-03 (14min), 02-01 (4min), 02-02 (5min)
- Trend: Phase 2 maintaining fast pace with focused infrastructure work

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Follow PRD closely — PRD contains detailed research and implementation guidance
- Regtest-only for this phase — Fast iteration, learn fundamentals before testnet complexity
- Pool modifications only — Validate core feasibility before building backend/frontend orchestration
- Localhost-only API — Simplify MVP, defer authentication to production hardening
- **01-01:** Separated type validation from address validation — bitcoin crate integration deferred to handler layer
- **01-01:** Empty user_id is valid — no minimum length constraint, only max 128 chars
- **01-01:** Descriptive validation error messages — improve API UX with detailed feedback
- **01-03:** Fixed bug over new implementation — existing update_coinbase_and_broadcast had critical job recreation bug, fixed it
- **01-03:** Added extended channel support — original implementation only handled standard channels
- **01-03:** Made coinbase_outputs pub(crate) — supports test infrastructure from Plan 01-02
- **01-03:** Added axum as direct dependency — unblocks http_api compilation
- **02-01:** reqwest over hyper for HTTP client — higher-level API with built-in connection pooling
- **02-01:** 5-second webhook timeout — prevents hanging on slow endpoints
- **02-01:** Fire-and-forget webhook pattern — tokio::spawn ensures pool operations never wait for HTTP responses
- **02-02:** Store mappings BEFORE broadcasting messages — atomic ordering prevents attribution gap
- **02-02:** Clear all mappings on SetNewPrevHash — simple strategy, jobs become stale on blockchain advance
- **02-02:** Track both group and standard channel job IDs — comprehensive coverage for all channel types

### Pending Todos

None yet.

### Blockers/Concerns

**Phase 1 Risk:**
- Merkle path validity after coinbase changes is a feasibility assumption requiring empirical validation. If TMPL-05 integration test fails (shares rejected after coinbase switch), the entire approach requires rearchitecture. This must be validated in Phase 3 before considering system production-ready.

**Build Environment (RESOLVED):**
- ~~cargo check fails due to missing capnp C++ headers~~ - User installed libcapnp-dev, now resolved
- Full pool compilation now works

## Session Continuity

Last session: 2026-02-05 20:21:43 UTC
Stopped at: Completed 02-02-PLAN.md (Job attribution mapping with atomic population and cleanup)
Resume file: None
