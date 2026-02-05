# Roadmap: Dynamic Coinbase Switching for SV2 Pool

## Overview

This roadmap delivers dynamic coinbase switching with share attribution in three phases. Phase 1 establishes the HTTP API foundation and validates the core feasibility assumption (merkle path reuse after coinbase changes). Phase 2 builds on this to add user tracking and webhook notifications. Phase 3 validates system stability under concurrent load and optimizes performance to meet sub-100ms requirements.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: API Foundation and Template Switching** - HTTP API with coinbase validation, job recreation and distribution
- [ ] **Phase 2: Share Attribution and Webhooks** - User tracking, webhook notifications, stale job cleanup
- [ ] **Phase 3: Concurrency Hardening and Performance** - Lock optimization, stress testing, sub-100ms validation

## Phase Details

### Phase 1: API Foundation and Template Switching
**Goal**: External control of coinbase addresses with validated job distribution to miners
**Depends on**: Nothing (first phase)
**Requirements**: API-01, API-02, API-03, API-04, API-05, TMPL-01, TMPL-02, TMPL-03, TMPL-04, CONC-01, CONC-02
**Success Criteria** (what must be TRUE):
  1. HTTP endpoint accepts POST /api/coinbase with address and user_id, returns 200 on success
  2. Invalid Bitcoin addresses rejected with 400 status (validated via bitcoin crate)
  3. Pool recreates jobs from stored template with new coinbase outputs within 100ms
  4. All connected miners receive NewMiningJob messages with updated coinbase
  5. Lock strategy prevents deadlock between API calls and template provider updates
**Plans**: TBD

Plans:
- [ ] 01-01: TBD during planning

### Phase 2: Share Attribution and Webhooks
**Goal**: Share notifications with correct user attribution for time-shared mining
**Depends on**: Phase 1
**Requirements**: WEBHOOK-01, WEBHOOK-02, WEBHOOK-03, WEBHOOK-04, WEBHOOK-05, WEBHOOK-06, WEBHOOK-07, WEBHOOK-08, WEBHOOK-09, CONC-03
**Success Criteria** (what must be TRUE):
  1. When share validated, webhook POST sent with user_id, share_hash, difficulty, job_id, timestamp
  2. Lagging shares (submitted after coinbase switch) attributed to correct user from job mapping
  3. Block solutions flagged with is_block=true in webhook payload
  4. Webhook delivery doesn't block pool operation (fire-and-forget pattern)
  5. Stale job mappings cleaned up on SetNewPrevHash to prevent memory leak
**Plans**: TBD

Plans:
- [ ] 02-01: TBD during planning

### Phase 3: Concurrency Hardening and Performance
**Goal**: System remains stable under concurrent load and meets sub-100ms performance target
**Depends on**: Phase 2
**Requirements**: TMPL-05, CONC-04
**Success Criteria** (what must be TRUE):
  1. Integration test validates shares accepted after coinbase switch (merkle path theory confirmed)
  2. Concurrent API calls and template updates complete without data races or deadlocks
  3. Lock contention measured and optimized to achieve sub-100ms job distribution
  4. Stress test with rapid API calls + high share rate shows no share rejection or attribution errors
**Plans**: TBD

Plans:
- [ ] 03-01: TBD during planning

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. API Foundation and Template Switching | 0/TBD | Not started | - |
| 2. Share Attribution and Webhooks | 0/TBD | Not started | - |
| 3. Concurrency Hardening and Performance | 0/TBD | Not started | - |

---
*Roadmap created: 2026-02-05*
*Depth: quick (3 phases)*
*Coverage: 27/27 requirements mapped*
