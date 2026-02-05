# Pitfalls Research

**Domain:** Mining pool internal modifications (SV2)
**Researched:** 2026-02-05
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Nested Lock Acquisition (Deadlock)

**What goes wrong:**
Acquiring a lock on `channel_manager_data` and then attempting to lock `downstream_data` (or vice versa) from within the same closure creates deadlock conditions. The pool becomes unresponsive, miners disconnect, and no shares can be processed.

**Why it happens:**
Rust developers naturally nest lock acquisitions when data structures are hierarchical (`ChannelManager` → `Downstream` → `DownstreamData`). The intuitive pattern `data.downstreams.get(id).lock()` called from within a `channel_manager_data.super_safe_lock()` creates lock ordering violations.

**How to avoid:**

- **Clone before descending**: When iterating over downstreams inside a channel manager lock, clone the collection first and release the parent lock before acquiring child locks.
- **Lock ordering discipline**: Establish a strict hierarchy: always acquire `channel_manager_data` locks BEFORE `downstream_data` locks, never the reverse.
- **Short critical sections**: Keep the scope of each lock minimal. Extract data while holding the lock, release it, then process the data.

**Warning signs:**

- Pool becomes unresponsive under load (multiple concurrent API calls + template updates)
- Logs show tasks stuck without progress
- `tokio-console` shows blocked tasks waiting on mutexes
- Integration tests hang intermittently

**Phase to address:**
Phase 1 (API foundation) — Before implementing the HTTP API endpoint, review all lock acquisition patterns in template handler and mining message handler. Add lock ordering tests.

**Evidence:**
Recent fix in commit `02ea18c7` demonstrates the pattern:

```rust
// DEADLOCK PATTERN (BAD):
self.sv1_server_data.super_safe_lock(|data| {
    for downstream in data.downstreams.values() {
        downstream.downstream_data.super_safe_lock(|d| {
            // nested lock acquisition
        });
    }
});

// CORRECT PATTERN:
let downstreams: Vec<_> = self
    .sv1_server_data
    .super_safe_lock(|data| data.downstreams.values().cloned().collect());

for downstream in &downstreams {
    downstream.downstream_data.super_safe_lock(|d| {
        // lock acquired after parent released
    });
}
```

---

### Pitfall 2: Future Job Synchronization Gap

**What goes wrong:**
When a new downstream connects while a future job exists but before `SetNewPrevHash` arrives, the downstream receives only the active job. When `SetNewPrevHash` arrives referencing the future job, the channel doesn't have it, causing share validation to fail with `InvalidJobId` errors. Miners receive rejected shares for valid work.

**Why it happens:**
Job activation happens asynchronously. A future template arrives (job ID 5), a downstream connects (receives job 4), then `SetNewPrevHash` arrives activating job 5. The downstream's channel never received job 5, so shares submitted against it are rejected.

**How to avoid:**

- When creating new downstream channels, populate BOTH `last_active_job` AND all `future_jobs` from the upstream state.
- Ensure `on_set_new_prev_hash` can transition a future job that already exists in the job store.
- Test with the scenario: `NewTemplate(future=true)` → `OpenChannel` → `SetNewPrevHash` → `SubmitShares`

**Warning signs:**

- `ShareValidationError::InvalidJobId` appearing in logs
- Share rejection rate higher than expected
- Miners complain about rejected shares immediately after connecting
- Integration tests fail when channels open between template and prev_hash messages

**Phase to address:**
Phase 1 (API foundation) — When implementing dynamic coinbase switching, the new job distribution logic must replicate this pattern: send both active and future jobs to all channels.

**Evidence:**
Fixed in commit `d9b8ca49`:

```rust
// Initialize the new downstream channel with state from upstream:
// chain tip, active job, and any pending future jobs.
let active_job = ch.get_active_job().map(|j| j.0.clone());
let futures = ch
    .get_future_jobs()
    .values()
    .map(|j| j.0.clone())
    .collect::<Vec<_>>();

// Also add any future jobs so SetNewPrevHash won't fail
for mut future_job in future_jobs {
    future_job.channel_id = next_channel_id;
    let _ = channel.on_new_extended_mining_job(future_job);
}
```

---

### Pitfall 3: Stale Job Mapping Memory Leak

**What goes wrong:**
Job-to-user mappings accumulate indefinitely. Each coinbase switch creates a new job with a new user_id mapping. Lagging miners submit shares against old jobs. If old job mappings are never cleaned up, memory grows unbounded until the pool OOMs and crashes.

**Why it happens:**
Shares can arrive seconds or even minutes after a job is issued (network latency, miner delay). You need to keep job mappings for "some time" to handle lagging shares. But without explicit cleanup, "some time" becomes "forever."

**How to avoid:**

- **Cleanup trigger**: `SetNewPrevHash` marks the blockchain tip change. All jobs from the previous block height become permanently stale at this point.
- **Job lifecycle states**:
  - `future`: Not yet active (future_template=true)
  - `active`: Current mining target (future job activated by SetNewPrevHash)
  - `stale`: Previous block height, still accept shares for ~30 seconds
  - `expired`: Remove from memory
- **TTL-based cleanup**: After `SetNewPrevHash`, schedule a cleanup task to purge jobs older than N seconds (e.g., 30s for lagging shares).
- **Track job generation**: Each `SetNewPrevHash` increments a "block height generation". Jobs from generation N-2 or older can be safely removed.

**Warning signs:**

- Pool memory usage grows continuously over hours/days
- `HashMap` or `Vec` sizes in job storage increase without bound
- Performance degradation over time as hash table lookups slow
- OOM crashes after extended operation

**Phase to address:**
Phase 2 (Share webhooks) — When implementing job-to-user tracking for share attribution, design the cleanup mechanism upfront. Do NOT defer to "we'll add it later."

**Evidence:**
From `template_distribution_message_handler.rs` line 165-166:

```rust
data.last_new_prev_hash = Some(msg.clone().into_static());
```

`SetNewPrevHash` is the natural cleanup boundary. Jobs from the previous `prev_hash` are now stale.

---

### Pitfall 4: Merkle Path Invalidation on Coinbase Change

**What goes wrong:**
Changing coinbase outputs invalidates the merkle path if the implementation caches merkle branches computed from the original coinbase. Miners submit shares with correct POW but invalid merkle roots. Pool rejects all shares, miners see 100% rejection rate and disconnect.

**Why it happens:**
Bitcoin block templates include a merkle path proving the coinbase connects to the merkle root. The merkle path is computed from `hash(coinbase) || merkle_path[0] || merkle_path[1] ...`. If coinbase outputs change, `hash(coinbase)` changes, but if `merkle_path` is cached from the original template, the recomputed merkle root will be wrong.

**How to avoid:**

- **Understand the architecture**: Determine whether the SV2 implementation recomputes the full merkle root on every share validation, or caches intermediate merkle branches.
- **Test the assumption**: Create a test that switches coinbase outputs and validates a share. If share validation fails with merkle root mismatch, merkle path is being cached incorrectly.
- **Correct implementation**: Either:
  1. Recompute merkle path when coinbase changes (safest, potentially slower)
  2. Store only transaction merkle paths (not including coinbase), recompute full merkle root on validation

**Warning signs:**

- Share validation errors citing merkle root mismatch
- All shares rejected after coinbase switch
- Error messages like "Invalid merkle root" or "Block hash doesn't match target"
- Miners disconnect immediately after coinbase update

**Phase to address:**
Phase 1 (API foundation) — BEFORE implementing the coinbase switching API, write a test validating shares after coinbase modification. This validates the core feasibility assumption.

**Evidence:**
From `template_distribution_message_handler.rs` line 43-44:

```rust
let mut coinbase_output = deserialize_outputs(channel_manager_data.coinbase_outputs.clone()).expect("deserialization failed");
coinbase_output[0].value = Amount::from_sat(msg.coinbase_tx_value_remaining);
```

Coinbase outputs are modified from template values. Need to verify this doesn't break merkle validation.

---

### Pitfall 5: Template Provider Race Condition

**What goes wrong:**
HTTP API call updates `coinbase_outputs` while template provider message handler reads `coinbase_outputs` to create jobs. Race condition causes:
1. Jobs sent with mismatched coinbase (half old, half new)
2. Panic from inconsistent state (some channels updated, others not)
3. Shares attributed to wrong user (job_id collision after state corruption)

**Why it happens:**
Two async tasks accessing shared mutable state without coordination:
- Task A (HTTP handler): `POST /set-coinbase` → locks `channel_manager_data`, updates `coinbase_outputs`
- Task B (template handler): `handle_new_template` → locks `channel_manager_data`, reads `coinbase_outputs`, creates jobs

If these overlap, jobs may be created with partially updated state.

**How to avoid:**

- **Atomic coinbase updates**: When updating coinbase, also regenerate and redistribute jobs in the same critical section.
- **Version epoch**: Attach a version number to `coinbase_outputs`. Jobs store the version they were created with. Reject shares if job version doesn't match current version.
- **Lock scope review**: Ensure `super_safe_lock` scope covers the entire update-and-redistribute operation, not just the state mutation.
- **Test with high concurrency**: Integration test that hammers the API with coinbase updates while template updates stream in from Bitcoin Core.

**Warning signs:**

- Intermittent share misattribution (wrong user gets credit)
- Jobs sent with coinbase output that doesn't match the API request
- Miners receive jobs with inconsistent parameters
- Errors only appear under load, not during single-threaded testing

**Phase to address:**
Phase 1 (API foundation) — API implementation must acquire the same lock as template handler and update atomically.

**Evidence:**
From reviewing `handle_new_template` in `template_distribution_message_handler.rs`:

```rust
let messages = self.channel_manager_data.super_safe_lock(|channel_manager_data| {
    let mut coinbase_output = deserialize_outputs(channel_manager_data.coinbase_outputs.clone())
        .expect("deserialization failed");
    coinbase_output[0].value = Amount::from_sat(msg.coinbase_tx_value_remaining);

    // ... creates jobs using coinbase_output
});
```

API handler must follow this exact pattern: lock, read template, modify coinbase, recreate jobs, send jobs, unlock.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip job cleanup "for now" | Faster MVP implementation | Memory leak leading to OOM | Never — cleanup is 20 lines of code |
| Use `unwrap()` inside `super_safe_lock` closures | Avoids error propagation boilerplate | Poisons the mutex on panic, deadlocks entire pool | Never — always return `Result` from closures |
| Global mutable state for user tracking | Simpler than per-job state | Can't handle concurrent jobs for same user, causes attribution bugs | Never — per-job state is the correct model |
| Cache merkle paths without validation | Performance optimization | Breaks on coinbase changes | Only if you test coinbase changes first |
| Test only single-threaded flows | Faster test execution | Misses all race conditions and deadlocks | Never for pool modifications — concurrency is core |

---

## Integration Gotchas

Common mistakes when connecting to external services.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Bitcoin Core (template provider) | Assuming templates arrive before shares | Buffer shares for N seconds if job not yet received, then reject stale |
| Webhook delivery | Fire-and-forget HTTP POST, lose shares if endpoint down | Retry with exponential backoff, DLQ for permanent failures |
| Address validation | Trust user-provided Bitcoin address without validation | Validate address format and network (regtest/testnet/mainnet) before accepting |
| Time synchronization | Use local system time for share timestamps | NTP sync required — timestamp drift causes nbits validation failures (see commit `34c865a8`) |

---

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Holding locks during I/O | Latency spikes, miner disconnections | Never hold a lock during HTTP call or file write | >10 concurrent API calls |
| Linear search through all jobs | Share validation slows logarithmically | Use `HashMap<JobId, Job>` indexed by job_id | >100 active jobs |
| Synchronous share webhooks | API call blocks until webhook completes | Use async channel to worker task for webhook delivery | Webhook endpoint latency >50ms |
| Unbounded channel buffers | Memory spikes under load | Use bounded channels with backpressure | Share rate >1000/sec |
| Clone entire job for every share | Memory allocation in hot path | Share validation borrows job immutably | Share rate >100/sec |

---

## Security Mistakes

Domain-specific security issues beyond general web security.

| Mistake | Risk | Prevention |
|---------|------|------------|
| No authentication on coinbase API | Anyone can redirect mining rewards | Bind to localhost only (MVP), bearer tokens for production |
| Accept any Bitcoin address format | Payments sent to invalid address, funds lost | Validate address with `bitcoin` crate, reject invalid formats |
| No rate limiting on API | DoS via rapid coinbase switches, miner disconnections | Rate limit to 1 request per second per IP |
| Leak user_id in share webhooks | Privacy violation if user_id is sensitive | Hash or anonymize user_id before sending to external webhook |
| Log user addresses in plaintext | Address correlation attack via log theft | Use structured logging with sensitive field redaction |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Job distribution**: Sent to group channel — verify extended channels and standard channels also receive it
- [ ] **Share validation**: Validates POW — verify merkle root, nbits, and job_id also validated
- [ ] **Coinbase switching**: Updates state — verify all connected channels receive new jobs within 100ms
- [ ] **User tracking**: Job-to-user mapping created — verify mapping cleaned up on SetNewPrevHash
- [ ] **Webhook delivery**: HTTP POST sent — verify retry logic, error handling, and timeout configured
- [ ] **Lock acquisition**: Lock acquired — verify lock released in all code paths (success, error, early return)
- [ ] **Channel types**: Works with extended channels — verify standard and group channels also work
- [ ] **Future job handling**: Active job sent to new channel — verify future jobs also sent

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Deadlock | MEDIUM | 1. Identify lock acquisition order from logs. 2. Refactor to acquire locks in consistent order. 3. Add lock order tests. |
| Future job gap | LOW | 1. Backport fix from commit `d9b8ca49`. 2. Add integration test from commit `78c6a273`. |
| Memory leak | MEDIUM | 1. Add cleanup trigger on `SetNewPrevHash`. 2. Implement job TTL (30s after stale). 3. Monitor memory metrics. |
| Merkle path issue | HIGH | 1. Investigate channels-sv2 merkle validation. 2. Write test for coinbase modification. 3. May require upstream fix. |
| Race condition | HIGH | 1. Add version epoch to coinbase state. 2. Refactor to atomic update-and-redistribute. 3. Add concurrency stress test. |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Nested lock acquisition | Phase 1 (API foundation) | Code review of all lock patterns, lock ordering test |
| Future job gap | Phase 1 (API foundation) | Integration test: connect channel between NewTemplate and SetNewPrevHash |
| Stale job cleanup | Phase 2 (Share webhooks) | Memory profiling over 1000+ job cycles, memory stays flat |
| Merkle path invalidation | Phase 1 (API foundation) | Unit test: modify coinbase, validate share, assert success |
| Template provider race | Phase 1 (API foundation) | Stress test: 100 concurrent API calls + template updates, no panics |

---

## Phase-Specific Warnings

Additional warnings for specific implementation phases.

### Phase 1: API Foundation (Coinbase Switching)

**High-risk areas:**
- Lock acquisition when updating `coinbase_outputs` — review all access to `channel_manager_data`
- Job recreation from stored template — ensure merkle path validity
- Distribution to all channel types (standard, extended, group) — test each independently

**Integration test requirements:**
- Merkle validation after coinbase change
- Concurrent template updates during API call
- Job received by all connected channels within 100ms

### Phase 2: Share Webhooks (User Tracking)

**High-risk areas:**
- Job-to-user mapping creation — ensure atomic with job distribution
- Cleanup on `SetNewPrevHash` — test memory stays bounded over 10,000 jobs
- Attribution of lagging shares — test shares arriving 30s after job

**Integration test requirements:**
- Share submitted after coinbase switch attributes to correct user
- Lagging share (old job_id) still attributes correctly
- Memory usage stable over 1000+ coinbase switches

### Phase 3: Production Hardening

**High-risk areas:**
- Authentication bypass via direct pool access (bind to 127.0.0.1)
- Webhook endpoint availability (implement retry with DLQ)
- Address validation (reject mainnet addresses on testnet)

---

## Sources

**Codebase analysis (HIGH confidence):**
- `/pool-apps/pool/src/lib/channel_manager/mod.rs` — Lock patterns, state management
- `/pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs` — Job lifecycle, future job handling
- `/pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs` — Share validation, channel management
- `/stratum-apps/src/custom_mutex.rs` — Lock safety patterns, documentation
- Commit `02ea18c7` — Deadlock fix via lock ordering
- Commit `d9b8ca49` — Future job synchronization fix
- Commit `78c6a273` — Integration test for future job handling
- Commit `34c865a8` — NTP requirement for time-sensitive validation

**Project context (HIGH confidence):**
- `.planning/PROJECT.md` — Technical unknowns: merkle path, concurrency, job lifecycle
- Git history — Recent bug fixes showing common pitfall patterns

**Domain knowledge (MEDIUM confidence):**
- Bitcoin mining protocol fundamentals (merkle path validation, share submission)
- Rust async patterns (lock ordering, deadlock prevention)
- SV2 protocol architecture (job lifecycle, channel types)

---

*Pitfalls research for: Dynamic coinbase switching in SV2 pool*

*Researched: 2026-02-05*

*Note: This research is based on codebase analysis and recent bug fix patterns. The three key technical unknowns (merkle path reuse, concurrency safety, job lifecycle) are addressed as Critical Pitfalls 4, 5, and 3 respectively. Phase 1 must validate Pitfall 4 (merkle path) before proceeding — this is a feasibility blocker.*
