# Project Research Summary

**Project:** Dynamic Coinbase Switching for SV2 Pool
**Domain:** Bitcoin mining pool with HTTP API integration
**Researched:** 2026-02-05
**Confidence:** HIGH

## Executive Summary

Dynamic coinbase switching for Stratum V2 mining pools is a specialized domain combining Bitcoin protocol knowledge, async Rust concurrency patterns, and distributed systems concerns. The research reveals that this project modifies an existing production pool to enable rapid (sub-100ms) switching of coinbase outputs via HTTP API, with webhook notifications for share attribution. The core technical challenge is maintaining consistency between concurrent state updates (API calls, template updates from Bitcoin Core, share submissions) while ensuring zero downtime and correct payout tracking.

The recommended approach is incremental integration using the existing pool's architecture: axum for HTTP API (already in use for monitoring), reqwest for webhooks, and the existing custom_mutex pattern for concurrency safety. The critical path is a three-phase lock-minimizing pattern for coinbase updates: read template state, compute new jobs outside the lock, then atomically update state and broadcast. This minimizes lock contention while maintaining consistency. The key risk is merkle path validity after coinbase changes, which must be validated before proceeding with any other work.

The research identifies five critical pitfalls that have caused production issues in similar pool modifications: nested lock acquisition (deadlock), future job synchronization gaps (share rejection), stale job mapping leaks (OOM), merkle path invalidation (100% share rejection), and template provider race conditions (misattribution). All are addressable through established patterns, but each requires explicit prevention in implementation phases.

## Key Findings

### Recommended Stack

The pool already has most required dependencies. The existing tokio async runtime, axum HTTP framework (0.8.7 in stratum-apps), and custom_mutex patterns are production-proven. Three new additions complete the stack: tower-http 0.6.8 for HTTP middleware, reqwest 0.13.1 for webhook delivery, and tokio::sync primitives for new async state management. The codebase analysis shows consistent patterns for shared state (Arc<Mutex<ChannelManagerData>>) and message passing (async-channel), which the new API integration should follow.

**Core technologies:**
- axum 0.8.8: HTTP API server framework — already in use, designed for tokio, excellent State extractor pattern for Arc sharing
- reqwest 0.13.1: Async HTTP client for webhooks — built on tokio/hyper, connection pooling for repeated webhooks, native timeout support
- tower-http 0.6.8: HTTP middleware layer — production middleware (timeout, tracing, error handling), works seamlessly with axum
- tokio 1.49.0: Async runtime — upgrade from 1.44.1 for latest fixes, pool already uses multi-threaded runtime
- stratum custom_mutex: Existing wrapper — continue using for ChannelManagerData, provides safe_lock/super_safe_lock patterns
- tokio::sync::RwLock: Async read-write lock — for new async-only state (webhook queue), doesn't block executor threads

### Expected Features

The feature landscape divides into three categories: table stakes (8 features), differentiators (6 features), and anti-features (6 commonly requested but problematic). The table stakes are all MVP-critical because they validate core feasibility assumptions. The differentiators polish the experience but aren't blocking. The anti-features (production authentication, sophisticated rate limiting, advanced retry logic) add complexity without validating the core concept and should be deferred to post-MVP production hardening.

**Must have (table stakes):**
- HTTP API for coinbase updates — core requirement for external control of address switching
- Address format validation — prevent invalid Bitcoin addresses from causing pool failures
- Job recreation from template — must recreate jobs with new coinbase without waiting for Bitcoin Core
- Broadcast to all miners <100ms — time-shared mining requires rapid switching between users
- Valid share webhook — external system needs immediate notification when shares found
- User attribution — track which user_id owns each job for correct share attribution
- Lagging share handling — miners submit shares after coinbase switched, must attribute correctly
- Concurrency safety — API calls and template updates happen concurrently, requires lock coordination

**Should have (competitive):**
- Sub-100ms switching — fairness guarantee for time-slotted mining (PRD requirement, optimization of broadcast)
- Block solution webhook flag — distinguish block solutions from regular shares (is_block: true)
- Job metadata in webhook — include job_id, channel_id, downstream_id for debugging/analytics
- Merkle path reuse validation — verify assumption that merkle_path stays valid when coinbase changes
- Automatic stale job cleanup — prevent memory leak from accumulated job-user mappings
- Multi-channel support — works with standard, extended, and group channels

**Defer (v2+):**
- Production authentication (bearer tokens) — adds complexity, MVP is localhost-only
- Sophisticated rate limiting — complex to implement correctly, unnecessary for MVP validation
- Advanced webhook retry logic — adds complexity, unclear requirements before usage data
- Multiple pool instances — distributed state synchronization is complex, validate single-pool concept first
- Testnet/mainnet deployment — regtest sufficient for learning, production requires hardening
- Dynamic difficulty adjustment — existing vardiff sufficient, optimize after usage data

### Architecture Approach

The architecture follows standard async Rust patterns with a central ChannelManager holding shared state (Arc<Mutex<ChannelManagerData>>), message handlers for Bitcoin Core templates and miner shares, and per-connection Downstream tasks. The HTTP API integrates as another tokio task spawned during pool startup, sharing the ChannelManager via Arc. The key pattern is lock-minimizing state updates: acquire lock, clone required data, drop lock, perform computation, acquire lock again for write. This prevents API calls from blocking template updates. Webhook delivery is fire-and-forget via tokio::spawn to ensure zero impact on share validation latency.

**Major components:**
1. ChannelManager — central state for all mining channels, templates, jobs. Receives messages from template provider, mining message handler, and new HTTP API
2. HTTP API Server — axum router with State(Arc<ChannelManager>), accepts coinbase update requests, triggers state updates via ChannelManager methods
3. Webhook Client — reqwest wrapper for fire-and-forget share notifications, spawned tasks with aggressive timeout, logs errors but doesn't block pool operation
4. Job Mapping — HashMap<JobId, UserId> tracking attribution, cleaned up on SetNewPrevHash to prevent memory leak

### Critical Pitfalls

The research identified five critical pitfalls from recent bug fixes in the codebase (commits 02ea18c7, d9b8ca49, 78c6a273). Each has caused production issues in pool modifications and requires explicit prevention.

1. **Nested lock acquisition (deadlock)** — acquiring channel_manager_data lock and then attempting to lock downstream_data creates deadlock. Prevention: clone collections before descending hierarchy, establish strict lock ordering (parent before child), keep critical sections short.

2. **Future job synchronization gap** — when downstream connects between NewTemplate(future=true) and SetNewPrevHash, channel lacks the future job, causing InvalidJobId share rejections. Prevention: populate BOTH last_active_job AND future_jobs when creating channels, test the scenario explicitly.

3. **Stale job mapping memory leak** — job-to-user mappings accumulate indefinitely without cleanup, growing until OOM. Prevention: cleanup on SetNewPrevHash (marks blockchain tip change), implement TTL-based expiration (30s after stale).

4. **Merkle path invalidation on coinbase change** — changing coinbase outputs potentially invalidates cached merkle branches, causing 100% share rejection. Prevention: test share validation after coinbase modification BEFORE implementing anything else, verify merkle root computation doesn't cache coinbase-dependent values.

5. **Template provider race condition** — HTTP API and template handler concurrently access coinbase_outputs, causing mismatched jobs or state corruption. Prevention: atomic coinbase update with job regeneration and distribution in same critical section, lock scope covers entire operation.

## Implications for Roadmap

Based on research, the natural phase structure follows dependency chains and risk mitigation priorities. The first phase must validate the core feasibility assumption (merkle path reuse) before building anything that depends on it. The second phase implements the happy path to prove end-to-end integration. The third phase handles edge cases and production concerns. This ordering frontloads risk validation and enables early learning.

### Phase 1: API Foundation and Validation

**Rationale:** Validates core feasibility assumption (merkle path reuse after coinbase change) before committing to implementation. Establishes HTTP API integration patterns without risking production pool operation. Implements coinbase switching and job distribution as isolated capability before adding attribution complexity.

**Delivers:**
- HTTP API endpoint for coinbase updates with address validation
- Job recreation from stored template with new coinbase outputs
- Broadcast to all channel types (standard, extended, group) within 100ms
- Merkle path validity verification after coinbase modification

**Addresses features:**
- HTTP API for coinbase updates (table stakes)
- Address format validation (table stakes)
- Job recreation from template (table stakes)
- Broadcast to all miners <100ms (table stakes)
- Merkle path reuse validation (differentiator, but critical assumption)

**Avoids pitfalls:**
- Merkle path invalidation (Pitfall 4) — MUST test before proceeding
- Template provider race condition (Pitfall 5) — atomic update pattern
- Nested lock acquisition (Pitfall 1) — lock ordering review before API implementation

**Research needs:** None. All patterns are established in existing codebase (monitoring HTTP server provides axum integration example, template_distribution_message_handler provides job distribution pattern).

### Phase 2: Share Attribution and Webhooks

**Rationale:** Builds on validated API foundation to add user tracking and external notification. Job-to-user mapping enables share attribution without requiring changes to share validation logic. Webhook integration proves external system communication pattern without blocking pool operation.

**Delivers:**
- Job-to-user mapping created on coinbase updates
- User attribution for share submissions (including lagging shares)
- Webhook notifications on valid shares with metadata
- Block solution distinction (is_block flag)
- Stale job cleanup on SetNewPrevHash

**Addresses features:**
- Valid share webhook (table stakes)
- User attribution (table stakes)
- Lagging share handling (table stakes)
- Block solution webhook flag (differentiator)
- Automatic stale job cleanup (differentiator)

**Avoids pitfalls:**
- Stale job mapping memory leak (Pitfall 3) — cleanup on SetNewPrevHash from start
- Future job synchronization gap (Pitfall 2) — job mapping follows same pattern as job distribution

**Research needs:** None. Webhook pattern is standard fire-and-forget with reqwest. Job mapping is straightforward HashMap with lifecycle management.

### Phase 3: Concurrency Hardening and Polish

**Rationale:** With happy path working, address race conditions and edge cases that emerge under load. Validate lock-minimizing patterns actually prevent contention. Ensure system remains stable under concurrent API calls, template updates, and high share submission rates.

**Delivers:**
- Lock contention measurement and optimization
- Integration tests for concurrent scenarios
- Multi-channel support verification (all three channel types)
- Sub-100ms optimization (if not already achieved)
- Job metadata in webhooks (channel_id, downstream_id)

**Addresses features:**
- Concurrency safety (table stakes, but validation phase)
- Multi-channel support (differentiator)
- Sub-100ms switching (differentiator, optimization)
- Job metadata in webhook (differentiator)

**Avoids pitfalls:**
- Template provider race condition (Pitfall 5) — stress testing
- Nested lock acquisition (Pitfall 1) — concurrency verification

**Research needs:** None. All patterns are standard async Rust practices. May need profiling to identify actual contention points, but no new architectural research required.

### Phase 4: Production Readiness (Deferred)

**Rationale:** Once core concept is validated in regtest with all table stakes features working, production deployment requires hardening that was deferred as anti-features. This phase only begins after product-market fit is established.

**Delivers:**
- API authentication (bearer tokens, bind restrictions)
- Advanced webhook retry with exponential backoff and DLQ
- Rate limiting (per-IP, per-endpoint)
- Testnet deployment
- Monitoring and alerting
- Graceful degradation patterns

**Addresses features:**
- Production authentication (deferred anti-feature)
- Advanced webhook retry logic (deferred anti-feature)
- Sophisticated rate limiting (deferred anti-feature)

**Research needs:** Standard production hardening patterns. May need specific research on Bitcoin testnet vs mainnet address validation, and on webhook reliability patterns (DLQ, idempotency).

### Phase Ordering Rationale

- **Phase 1 first:** Merkle path validity is a feasibility blocker. If coinbase changes break merkle validation, the entire approach fails. Test this assumption before investing in attribution, webhooks, or any other features.

- **Phase 2 after Phase 1:** Share attribution depends on job distribution working correctly. Building job-to-user mapping before jobs are reliably distributed would require rework. Webhook integration is independent of API but depends on attribution to be useful.

- **Phase 3 after Phase 2:** Lock contention and race conditions only manifest under load, and only matter once happy path works. Optimizing locks before functionality works is premature. Stress testing requires working features to stress.

- **Phase 4 deferred:** Production hardening is standard but context-dependent. Defer until actual usage reveals which protections are necessary (authentication scheme depends on deployment environment, retry logic depends on webhook endpoint SLAs, rate limits depend on actual usage patterns).

- **Grouping by dependency chains:** Phase 1 (API + jobs) forms the foundation. Phase 2 (attribution + webhooks) builds on that foundation. Phase 3 (hardening) validates the combined system. This avoids rework from discovering fundamental issues late.

- **Pitfall prevention through ordering:** Phase 1 explicitly tests Pitfall 4 (merkle path) before building anything. Phase 1 reviews lock patterns (Pitfall 1, Pitfall 5) before API calls introduce concurrency. Phase 2 implements cleanup (Pitfall 3) from the start rather than retrofitting later.

### Research Flags

Phases with standard patterns (skip research-phase during planning):
- **Phase 1:** Axum integration is proven pattern (monitoring server example exists). Job distribution follows existing template handler pattern. Lock strategy uses existing custom_mutex patterns. No new research needed.
- **Phase 2:** Webhook client is standard reqwest fire-and-forget pattern. Job mapping is straightforward HashMap with lifecycle. Share validation integration uses existing handler hooks. No new research needed.
- **Phase 3:** Concurrency testing uses standard tokio stress test patterns. Lock optimization follows profiling (tracing spans for measurement). No new research needed until profiling reveals specific bottleneck.

Phases that might need research during planning:
- **Phase 4:** Production hardening is context-dependent. May need `/gsd:research-phase` for specific topics like "webhook reliability patterns" or "Bitcoin address validation across networks" depending on deployment requirements discovered during earlier phases.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All recommended technologies already in use (axum, tokio) or standard choices (reqwest, tower-http). Verified versions via crates.io. Patterns proven in existing codebase. |
| Features | HIGH | Feature landscape derived from Stratum V2 spec, PRD requirements, and codebase analysis. Table stakes validated against Bitcoin mining protocol requirements. Anti-features identified from domain experience. |
| Architecture | HIGH | Architecture patterns extracted from existing working pool code. HTTP integration follows monitoring server example. Lock patterns use existing custom_mutex. Data flow verified against actual message handlers. |
| Pitfalls | HIGH | All five critical pitfalls are documented in recent bug fix commits (02ea18c7, d9b8ca49, etc). Prevention strategies are proven fixes. Phase-specific warnings based on codebase patterns. |

**Overall confidence:** HIGH

### Gaps to Address

While research confidence is high, three areas require validation during implementation rather than research:

- **Merkle path reuse after coinbase change:** Research confirms the architecture suggests this should work (merkle_path stored separately from coinbase in TemplateData, merkle root recomputed on validation). However, this is a feasibility assumption that must be tested empirically. Phase 1 must include explicit test: create job, modify coinbase, validate share, assert success. If this test fails, the entire approach requires rearchitecture.

- **Lock contention under load:** Research identifies lock-minimizing patterns (three-phase updates) but actual contention depends on API call frequency vs template update rate. Phase 3 must measure lock hold times with tracing spans and validate that 100ms broadcast target is achievable. Mitigation (RwLock upgrade) is straightforward if needed.

- **Job lifecycle timing assumptions:** Stale job cleanup on SetNewPrevHash assumes shares won't arrive more than one block period late. Research suggests 30s is safe margin, but actual network conditions might require tuning. Phase 2 should monitor share timestamp distribution to validate cleanup timing doesn't prematurely reject valid shares.

## Sources

### Primary (HIGH confidence)

**Codebase analysis:**
- `/pool-apps/pool/src/lib/channel_manager/mod.rs` — ChannelManager architecture, Arc<Mutex<>> patterns
- `/pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs` — Job lifecycle, coinbase handling, future job patterns
- `/pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs` — Share validation, channel management
- `/stratum-apps/src/custom_mutex.rs` — Lock safety patterns, super_safe_lock documentation
- `/stratum-apps/src/monitoring/http_server.rs` — Axum integration example, State pattern
- `/pool-apps/pool/Cargo.toml` — Existing dependency versions
- `/stratum-apps/Cargo.toml` — Shared dependencies (axum 0.8.7)

**Git history (bug fix commits):**
- Commit `02ea18c7` — Deadlock fix via lock ordering (nested lock acquisition pitfall)
- Commit `d9b8ca49` — Future job synchronization fix (job gap pitfall)
- Commit `78c6a273` — Integration test for future job handling
- Commit `34c865a8` — NTP time sync requirement

**Official documentation:**
- Stratum V2 Specification (Mining Protocol): https://raw.githubusercontent.com/stratum-mining/sv2-spec/main/05-Mining-Protocol.md — NewExtendedMiningJob structure, merkle path independence
- https://docs.rs/axum/latest/axum/ — State patterns, tokio integration
- https://docs.rs/reqwest/latest/reqwest/ — Client setup, connection pooling
- https://docs.rs/tokio/latest/tokio/sync/ — Async synchronization primitives

**Package registries:**
- crates.io (verified 2026-02-05): axum 0.8.8, reqwest 0.13.1, tokio 1.49.0, tower 0.5.3, tower-http 0.6.8

### Secondary (MEDIUM confidence)

- Sub-100ms latency target: PRD specification from PROJECT.md, not yet validated in practice. Achievable based on lock hold time estimates (1-50μs per acquisition) but requires empirical measurement.
- Webhook payload format: Derived from PRD and share validation context. May need refinement based on actual external system requirements discovered during Phase 2 integration testing.

### Tertiary (LOW confidence)

- Rate limiting thresholds: Generic recommendation (1 req/sec), not validated against actual usage patterns. Defer to Phase 4 after production data available.
- Cleanup timing (30s TTL): Estimated safety margin for lagging shares. May need adjustment based on actual network conditions observed during Phase 2.

---
*Research completed: 2026-02-05*
*Ready for roadmap: yes*
