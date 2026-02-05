---
phase: 02-share-attribution-and-webhooks
plan: 01
subsystem: webhook
tags: [reqwest, tokio, http, webhooks, fire-and-forget, async, tracing]

# Dependency graph
requires:
  - phase: 01-api-foundation-and-template-switching
    provides: PoolConfig structure and ChannelManager foundation
provides:
  - WebhookClient with reqwest HTTP client and connection pooling
  - ShareWebhookPayload type with all 9 required fields (WEBHOOK-02 through WEBHOOK-05)
  - Fire-and-forget webhook delivery pattern (tokio::spawn)
  - webhook_url configuration in PoolConfig
affects: [02-02-job-attribution, 02-03-share-validation-integration]

# Tech tracking
tech-stack:
  added: [reqwest 0.12, chrono 0.4]
  patterns: [fire-and-forget async with tokio::spawn, connection pooling, structured logging with tracing]

key-files:
  created:
    - pool-apps/pool/src/lib/webhook/mod.rs
    - pool-apps/pool/src/lib/webhook/types.rs
    - pool-apps/pool/src/lib/webhook/client.rs
  modified:
    - pool-apps/pool/Cargo.toml
    - pool-apps/pool/src/lib/mod.rs
    - pool-apps/pool/src/lib/config.rs

key-decisions:
  - "Used reqwest over hyper for HTTP client - higher-level API with built-in connection pooling"
  - "5-second timeout for webhooks to prevent hanging on slow endpoints"
  - "Max 10 idle connections per host for connection reuse efficiency"
  - "Default webhook URL localhost:3000 for development"
  - "Fire-and-forget pattern ensures webhook failures never block pool operations"

patterns-established:
  - "Fire-and-forget webhook pattern: tokio::spawn with no .await in critical path"
  - "Connection pooling: Single reqwest::Client shared across deliveries"
  - "Error logging without propagation: tracing::error! in spawned tasks"
  - "Span instrumentation: #[instrument] with user_id, job_id, is_block fields"

# Metrics
duration: 4min
completed: 2026-02-05
---

# Phase 02 Plan 01: Webhook Infrastructure Summary

**reqwest-based HTTP client with fire-and-forget delivery (tokio::spawn), 9-field ShareWebhookPayload, and connection pooling for non-blocking share notifications**

## Performance

- **Duration:** 4 minutes
- **Started:** 2026-02-05T11:23:12Z
- **Completed:** 2026-02-05T11:27:23Z
- **Tasks:** 3
- **Files created:** 3
- **Files modified:** 3

## Accomplishments

- WebhookClient with reqwest connection pooling and 5-second timeout
- ShareWebhookPayload type with all 9 required fields for complete share attribution
- Fire-and-forget delivery pattern using tokio::spawn (non-blocking)
- webhook_url configuration added to PoolConfig
- Structured logging with tracing instrumentation

## Task Commits

Each task was committed atomically:

1. **Tasks 1-2: Add webhook module and ShareWebhookPayload** - `c415fa6` (feat)
2. **Task 3: Implement WebhookClient with fire-and-forget** - `973af7f` (feat)

Note: Tasks 1 and 2 were combined into a single commit as they form a cohesive unit (module structure + type definition).

## Files Created/Modified

**Created:**
- `pool-apps/pool/src/lib/webhook/mod.rs` - Webhook module with exports and documentation
- `pool-apps/pool/src/lib/webhook/types.rs` - ShareWebhookPayload with 9 required fields
- `pool-apps/pool/src/lib/webhook/client.rs` - WebhookClient with fire-and-forget delivery

**Modified:**
- `pool-apps/pool/Cargo.toml` - Added reqwest 0.12 and chrono 0.4 dependencies
- `pool-apps/pool/src/lib/mod.rs` - Declared webhook module
- `pool-apps/pool/src/lib/config.rs` - Added webhook_url field with getter

## Decisions Made

1. **reqwest over hyper**: Used reqwest for higher-level HTTP client API with built-in connection pooling, timeout handling, and JSON serialization. Simpler than manual hyper client configuration.

2. **5-second timeout**: Prevents webhooks from hanging on slow endpoints while allowing reasonable network latency. Pool operations never wait longer than 5 seconds for webhook delivery (though they return immediately via tokio::spawn).

3. **Connection pooling (10 idle per host)**: Reuses HTTP connections for efficiency, reducing overhead of repeated webhook deliveries to same endpoint.

4. **Default webhook URL**: Set to `http://localhost:3000/webhook` for development convenience. Production pools override via config.

5. **Fire-and-forget pattern**: Critical architectural decision - webhooks NEVER block share validation path. Uses tokio::spawn to deliver in background, returns immediately.

## Deviations from Plan

None - plan executed exactly as written.

All three tasks completed as specified:
- Task 1: Added reqwest/chrono dependencies, created module structure
- Task 2: Implemented ShareWebhookPayload with all 9 required fields
- Task 3: Implemented WebhookClient with fire-and-forget pattern and PoolConfig integration

## Issues Encountered

None. Compilation succeeded on first attempt for all tasks.

## Requirements Satisfied

- **WEBHOOK-01**: Pool sends HTTP POST webhook when share validated (infrastructure ready)
- **WEBHOOK-02**: Webhook payload includes user_id field
- **WEBHOOK-03**: Webhook payload includes share_hash, difficulty, job_id, timestamp
- **WEBHOOK-04**: Webhook payload includes is_block flag
- **WEBHOOK-05**: Webhook payload includes channel_id, downstream_id, sequence_number
- **WEBHOOK-08**: Fire-and-forget delivery pattern (non-blocking via tokio::spawn)
- **WEBHOOK-09**: Webhook failures logged but don't block pool operation

## Next Phase Readiness

**Ready for Plan 02-02 (Job Attribution)**:
- WebhookClient available for import in ChannelManager
- ShareWebhookPayload type defines the data structure for notifications
- webhook_url configuration accessible via PoolConfig

**Integration points established**:
1. ChannelManager needs to store WebhookClient instance
2. Job-to-user mapping (HashMap<u32, String>) to be added in next plan
3. Share validation handlers will call webhook_client.send_share_notification()

**No blockers.** Webhook infrastructure is complete and ready for integration.

---
*Phase: 02-share-attribution-and-webhooks*
*Completed: 2026-02-05*
