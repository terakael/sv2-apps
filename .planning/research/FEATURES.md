# Feature Landscape: Dynamic Coinbase Switching for SV2 Pool

**Domain:** Bitcoin mining pool with dynamic coinbase address switching
**Researched:** 2026-02-05
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **HTTP API for coinbase updates** | Core requirement for external control of address switching | LOW | Simple REST endpoint with address + user_id |
| **Address format validation** | Prevent invalid Bitcoin addresses from causing pool failures | LOW | Use bitcoin crate's Address::from_str validation |
| **Job recreation from template** | Must recreate jobs with new coinbase without waiting for Bitcoin Core | MEDIUM | Requires understanding template structure (prefix, suffix, merkle_path) |
| **Broadcast to all miners <100ms** | Time-shared mining requires rapid switching between users | MEDIUM | Must distribute NewExtendedMiningJob to all active channels |
| **Valid share webhook** | External system needs immediate notification when shares found | LOW | HTTP POST with share metadata (hash, difficulty, timestamp) |
| **User attribution** | Track which user_id owns each job for correct share attribution | MEDIUM | Maintain job_id → user_id mapping with cleanup |
| **Lagging share handling** | Miners submit shares after coinbase switched, must attribute correctly | MEDIUM | Keep historical job mappings until SetNewPrevHash |
| **Concurrency safety** | API calls and template updates from Bitcoin Core happen concurrently | HIGH | Lock coordination between HTTP handler and template provider |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Sub-100ms switching** | Fairness guarantee for time-slotted mining (meets PRD requirement) | MEDIUM | Requires optimized job creation path, pre-allocated structures |
| **Block solution webhook flag** | Distinguish block solutions from regular shares (`is_block=true`) | LOW | Check ShareValidationResult::BlockFound variant |
| **Job metadata in webhook** | Include job_id, channel_id, downstream_id for debugging/analytics | LOW | Extract from share validation context |
| **Merkle path reuse validation** | Verify assumption that merkle_path stays valid when coinbase changes | MEDIUM | Critical assumption for feasibility - requires testing |
| **Automatic stale job cleanup** | Prevent memory leak from accumulated job-user mappings | LOW | Clear mappings on SetNewPrevHash (new block) |
| **Multi-channel support** | Works with standard, extended, and group channels | MEDIUM | PRD requires all channel types, existing pool supports this |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Production authentication** | Security for API endpoint | Adds complexity, slows MVP validation | Bind to localhost only, defer to production hardening phase |
| **Sophisticated rate limiting** | Prevent API abuse | Complex to implement correctly, unnecessary for MVP | Basic protection only (single request per 10ms), validate core feasibility first |
| **Advanced webhook retry logic** | Ensure notification delivery | Adds complexity, unclear requirements before usage data | Simple exponential backoff sufficient, iterate based on production needs |
| **External share validation** | Validate shares match expected user | Complicates architecture, trust boundary unclear | Pool validates internally, external system trusts webhook |
| **Multiple pool instances** | High availability, load balancing | Distributed state synchronization is complex | Single pool instance for MVP, validate core concept first |
| **Mainnet deployment** | "Real" testing | Too risky for learning phase, slow iteration | Regtest only - fast, safe, validates all mechanics |

## Feature Dependencies

```
[Address Validation]
    └──requires──> [HTTP API Endpoint]

[Job Recreation]
    └──requires──> [Template Structure Understanding]
    └──requires──> [Merkle Path Reuse Validation] ← CRITICAL ASSUMPTION

[Broadcast to Miners]
    └──requires──> [Job Recreation]
    └──requires──> [ChannelManager Integration]

[User Attribution]
    └──requires──> [Job-to-User Mapping]
    └──requires──> [Lagging Share Handling]

[Share Webhook]
    └──requires──> [User Attribution]
    └──requires──> [Share Validation Integration]

[Concurrency Safety]
    └──requires──> [Lock Strategy Design]
    └──blocks──> [All Features] ← Must solve this early

[Stale Job Cleanup]
    └──requires──> [SetNewPrevHash Handler]
```

### Dependency Notes

- **Merkle Path Reuse is CRITICAL:** The entire approach depends on the assumption that when coinbase outputs change, the merkle_path (sibling hashes from other transactions) remains valid. From codebase analysis (template_data.rs:155-166), the merkle root is computed by combining coinbase TXID with sibling hashes. Changing coinbase changes TXID but NOT the merkle path. **Status: Verified from codebase, needs runtime testing to confirm.**

- **Concurrency is HIGH RISK:** The ChannelManager is protected by Arc<Mutex<>> and receives messages from both HTTP API (new thread) and template provider (existing task). Improper locking can cause deadlock or data races. Must design lock strategy early.

- **Job-to-User Mapping lifecycle:** Jobs created → attributed to user_id → miners submit shares (potentially seconds later) → new coinbase switch → must still attribute old shares correctly → SetNewPrevHash (new block) → safe to clean up old mappings.

## Technical Validations Needed

Critical assumptions that must be validated during implementation:

| Assumption | Why It Matters | How to Validate | Risk Level |
|------------|----------------|-----------------|------------|
| **Merkle path reuse** | Entire approach depends on NOT needing new merkle_path when coinbase changes | Create job, change coinbase outputs, verify share validation still works | CRITICAL |
| **Lock-free broadcasting** | Sub-100ms requires minimal lock contention | Measure broadcast time under concurrent API calls + template updates | HIGH |
| **Job ID uniqueness** | Job IDs must be unique across coinbase switches for correct attribution | Test rapid switches, verify no ID collisions | MEDIUM |
| **Share validation with modified coinbase** | Pool's validate_share must work with reconstructed jobs | Submit shares after coinbase switch, verify acceptance | HIGH |
| **Template structure assumptions** | Assumptions about NewTemplate message fields (coinbase_prefix, coinbase_tx_outputs, merkle_path) | Review SV2 spec, test with real Bitcoin Core templates | MEDIUM |

## MVP Definition

### Launch With (v1)

Minimum viable product — what's needed to validate the concept.

- [x] **HTTP API endpoint** — POST /api/v1/coinbase with {address, user_id}
- [x] **Address validation** — Reject invalid Bitcoin addresses immediately
- [x] **Job recreation from stored template** — Modify coinbase outputs, keep merkle_path
- [x] **Broadcast to all channels** — NewExtendedMiningJob to standard, extended, group
- [x] **Share webhook on validation** — POST to configured URL with share metadata
- [x] **User attribution** — job_id → user_id mapping in ChannelManagerData
- [x] **Lagging share handling** — Keep mappings until SetNewPrevHash
- [x] **Concurrency safety** — Lock strategy prevents deadlock/races
- [x] **Sub-100ms broadcast** — Meets fairness requirement for time-slotted mining
- [x] **Block solution flag** — is_block: true in webhook for BlockFound

**Why these are essential:**
Each feature validates a core assumption about feasibility. Cannot validate time-shared mining concept without all pieces working together.

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] **Webhook retry logic** — Trigger: First production failure due to network issue
- [ ] **API rate limiting** — Trigger: Load testing reveals abuse potential
- [ ] **Metrics/monitoring** — Trigger: Need observability for production operations
- [ ] **Graceful degradation** — Trigger: Understanding failure modes from production usage
- [ ] **Historical job cleanup tuning** — Trigger: Memory profiling reveals optimization opportunities

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] **Production authentication (bearer tokens)** — Why defer: Adds complexity, MVP is localhost-only
- [ ] **Multiple pool instances** — Why defer: Distributed state is complex, validate single-pool concept first
- [ ] **Testnet/mainnet deployment** — Why defer: Regtest sufficient for learning, production requires hardening
- [ ] **Advanced webhook formats** — Why defer: Iterate based on actual external system requirements
- [ ] **Dynamic difficulty adjustment** — Why defer: Existing vardiff sufficient, optimize after usage data

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority | Notes |
|---------|------------|---------------------|----------|-------|
| Merkle path validation | CRITICAL | LOW | P0 | Blocks everything, validate first |
| HTTP API endpoint | HIGH | LOW | P1 | Entry point for all functionality |
| Job recreation | HIGH | MEDIUM | P1 | Core mechanic, depends on merkle validation |
| Concurrency safety | HIGH | HIGH | P1 | Must solve early, affects all features |
| Address validation | HIGH | LOW | P1 | Prevent pool crashes from bad input |
| User attribution | HIGH | MEDIUM | P1 | Required for share webhook usefulness |
| Broadcast to miners | HIGH | MEDIUM | P1 | Required for miners to receive work |
| Share webhook | HIGH | LOW | P1 | External system notification |
| Lagging share handling | MEDIUM | MEDIUM | P1 | Fairness requirement, handles real-world timing |
| Sub-100ms broadcast | MEDIUM | MEDIUM | P2 | Optimization of P1 broadcast feature |
| Block solution flag | MEDIUM | LOW | P2 | Nice-to-have distinction for external system |
| Stale job cleanup | LOW | LOW | P2 | Prevents memory leak, not blocking |
| Webhook retry | LOW | MEDIUM | P3 | Defer until production usage |
| Rate limiting | LOW | MEDIUM | P3 | Defer until load testing |

**Priority key:**
- P0: Must validate before proceeding (assumptions)
- P1: Must have for launch (MVP)
- P2: Should have, add when possible (post-MVP polish)
- P3: Nice to have, future consideration (production hardening)

## Implementation Sequence

Based on dependencies and risk:

1. **Phase 0: Validation** (P0 items)
   - Validate merkle path reuse assumption with test
   - Document template structure from codebase analysis
   - Design lock strategy (sketch, don't implement yet)

2. **Phase 1: Core Path** (P1 items - happy path)
   - HTTP API endpoint (basic)
   - Address validation
   - Job recreation from template
   - Broadcast to miners (single channel type first)
   - Basic concurrency (simple mutex, optimize later)

3. **Phase 2: Production-Ready** (P1 items - edge cases)
   - User attribution + job mapping
   - Lagging share handling
   - Share webhook integration
   - Multi-channel support (standard, extended, group)
   - Concurrency refinement (measure + optimize)

4. **Phase 3: Polish** (P2 items)
   - Sub-100ms optimization (if not already achieved)
   - Block solution webhook flag
   - Stale job cleanup
   - Integration testing across all scenarios

## Edge Cases to Handle

| Edge Case | Scenario | Handling Strategy |
|-----------|----------|------------------|
| **Concurrent API calls** | Two coinbase updates arrive simultaneously | Serialize with mutex, last-write-wins on user_id |
| **Template update during switch** | Bitcoin Core sends SetNewPrevHash while API updating coinbase | Lock ordering: template provider → API, prevent deadlock |
| **Miner disconnect during switch** | Downstream disconnects mid-broadcast | Non-blocking send, log failure, continue to other miners |
| **Invalid user_id** | API receives empty or malformed user_id | Validate at API boundary, return 400 error |
| **Share for unknown job** | Miner submits share for job_id not in mapping | Treat as stale share, return error to miner |
| **Memory leak from job mappings** | Mappings accumulate without cleanup | Clear on SetNewPrevHash (new block template) |
| **Webhook endpoint down** | Share webhook POST fails | Log error, continue pool operation (fire-and-forget) |
| **Zero coinbase value** | Template has zero coinbase_tx_value_remaining | Validation error, reject template from Bitcoin Core |

## Sources

**HIGH Confidence:**
- Stratum V2 Specification (Mining Protocol): https://raw.githubusercontent.com/stratum-mining/sv2-spec/main/05-Mining-Protocol.md
  - Verified: NewExtendedMiningJob structure (coinbase_tx_prefix, coinbase_tx_suffix, merkle_path)
  - Verified: Merkle path is separate from coinbase transaction fields

- Codebase Analysis (bitcoin-core-sv2/src/template_data.rs):
  - Lines 155-166: Merkle root calculation shows merkle_path sibling hashes are independent of coinbase TXID
  - Lines 72-90: NewTemplate message construction separates coinbase fields from merkle_path
  - Lines 28-48: TemplateData stores merkle_path separately from coinbase_tx

- Codebase Analysis (pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs):
  - Lines 519-620: Share validation flow shows available data (downstream_id, channel_id, sequence_number, share_hash)
  - Lines 560-603: ShareValidationResult variants (Valid, BlockFound) provide attribution context

**MEDIUM Confidence:**
- Sub-100ms switching requirement: PRD specification (from PROJECT.md), not yet validated in practice
- Concurrency patterns: Inferred from codebase architecture (Arc<Mutex<>> patterns), actual lock contention not measured

**LOW Confidence:**
- Webhook payload requirements: Derived from PRD and inferred from share validation context, may need refinement based on external system needs
- Rate limiting approach: Generic recommendation, needs load testing to validate necessity and threshold

---

*Feature research for: Dynamic Coinbase Switching for SV2 Pool*
*Researched: 2026-02-05*
