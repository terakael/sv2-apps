# Dynamic Coinbase Switching for SV2 Pool

## What This Is

A dynamic coinbase address switching and share notification system for the SV2 mining pool. Enables rapid (sub-100ms) switching of Bitcoin reward addresses without waiting for new templates from Bitcoin Core, supporting time-shared mining where multiple users mine with their own addresses on rotating time slots.

## Core Value

Sub-100ms coinbase switching enables fair time-shared mining by allowing rapid address rotation while miners continuously work, without waiting for Bitcoin Core template updates.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] HTTP API endpoint accepts Bitcoin address and user_id
- [ ] API validates address format and user_id constraints
- [ ] Coinbase outputs updated in ChannelManager state
- [ ] Jobs recreated from stored template with new coinbase
- [ ] New jobs distributed to all connected miners (standard, extended, group channels)
- [ ] Miners receive updated jobs within 100ms of API call
- [ ] Share webhook notifications sent when shares validated by pool
- [ ] Webhook payload includes user_id, share hash, difficulty, job_id, timestamp
- [ ] User ID tracked per job for correct share attribution
- [ ] Job-to-user mapping handles lagging shares (submitted after coinbase switch)
- [ ] Stale job mappings cleaned up on SetNewPrevHash (prevents memory leak)
- [ ] Merkle path remains valid when coinbase outputs change (validate assumption)
- [ ] Concurrent template updates from Bitcoin Core handled safely
- [ ] Lock strategy prevents deadlock between API calls and template provider
- [ ] Block solutions trigger webhook with is_block=true flag

### Out of Scope

- Production authentication (bearer tokens) — localhost-only binding for MVP
- Sophisticated rate limiting — basic protection only
- Advanced webhook retry logic — simple exponential backoff sufficient
- External share validation — removed from this phase, validate internally only
- Multiple pool instances / distributed deployment
- Testnet or mainnet deployment — regtest only for learning
- Frontend application — separate project
- Backend orchestration API — separate project
- Sidecar proxy component — separate project

## Context

**Existing Codebase:**
- Modifying sv2-apps pool implementation (Rust)
- Pool already handles template distribution, channel management, share validation
- Key integration points: ChannelManager, template_distribution_message_handler, mining_message_handler

**Learning Focus:**
- Deep dive into Bitcoin mining internals: template mechanics, merkle paths, coinbase construction
- Understanding share validation: difficulty targets, hash verification
- SV2 protocol flows: job distribution, channel types, message handling
- Rust async patterns, concurrency, ownership

**Technical Unknowns to Validate:**
1. Merkle path reuse: Does changing coinbase outputs keep merkle_path valid?
2. Concurrency safety: How to coordinate API calls with template provider messages?
3. Job lifecycle: When/how do jobs transition between future/past/stale states?

**Foundation for Future:**
- Backend API will orchestrate coinbase switching via fairness algorithm
- Frontend will provide user dashboard with Lightning authentication
- This phase validates core feasibility before building orchestration layers

## Constraints

- **Tech Stack**: Rust, existing sv2-apps codebase architecture
- **Timeline**: Learning pace — deep understanding prioritized over speed
- **Rust Experience**: Learning as going — will need to grasp ownership, async, concurrency patterns
- **Testing Environment**: Bitcoin Core regtest only (local, fast iteration)
- **Integration Requirement**: Must work with existing ChannelManager, template provider, all channel types (standard, extended, group)
- **Dependencies**: Existing sv2 protocol libraries, channels-sv2, bitcoin crate

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Follow PRD closely | PRD contains detailed research and implementation guidance | — Pending |
| Regtest-only for this phase | Fast iteration, learn fundamentals before testnet complexity | — Pending |
| Pool modifications only | Validate core feasibility before building backend/frontend orchestration | — Pending |
| Localhost-only API | Simplify MVP, defer authentication to production hardening | — Pending |

---
*Last updated: 2026-02-05 after initialization*
