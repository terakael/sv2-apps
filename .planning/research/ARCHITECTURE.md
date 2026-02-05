# Architecture Research: HTTP API Integration with SV2 Pool

**Domain:** Async Rust HTTP API integration with existing tokio-based actor system
**Researched:** 2026-02-05
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
┌────────────────────────────────────────────────────────────────────┐
│                       Application Layer                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │
│  │  main.rs     │  │  HTTP API    │  │  Monitoring  │            │
│  │  (entry)     │  │  Server      │  │  Server      │            │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘            │
│         │                 │                 │                      │
├─────────┴─────────────────┴─────────────────┴──────────────────────┤
│                       Business Logic Layer                          │
│  ┌──────────────────────────────────────────────────────────┐     │
│  │              ChannelManager (central state)               │     │
│  │  Arc<Mutex<ChannelManagerData>>                          │     │
│  │  - downstream: HashMap<DownstreamId, Downstream>         │     │
│  │  - coinbase_outputs: Vec<u8>                             │     │
│  │  - last_future_template: Option<NewTemplate>             │     │
│  │  - last_new_prev_hash: Option<SetNewPrevHash>            │     │
│  │  - job_to_user_mapping: HashMap<JobId, UserId> (NEW)     │     │
│  └──────────────────────────────────────────────────────────┘     │
│         ↑                    ↑                    ↑                │
│         │                    │                    │                │
│  ┌──────┴────────┐  ┌────────┴────────┐  ┌────────┴────────┐     │
│  │   Template    │  │   Mining Msg    │  │   Downstream    │     │
│  │   Receiver    │  │   Handler       │  │   Connections   │     │
│  └───────────────┘  └─────────────────┘  └─────────────────┘     │
├─────────────────────────────────────────────────────────────────────┤
│                       Communication Layer                           │
│  ┌─────────────────────────────────────────────────────────┐       │
│  │  async_channel (unbounded)  +  broadcast (bounded)      │       │
│  │  - tp_sender / tp_receiver                              │       │
│  │  - downstream_sender / downstream_receiver              │       │
│  │  - shutdown_broadcast                                   │       │
│  └─────────────────────────────────────────────────────────┘       │
├─────────────────────────────────────────────────────────────────────┤
│                       Network Layer                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐            │
│  │  Bitcoin     │  │  Noise TCP   │  │  HTTP        │            │
│  │  Core IPC    │  │  (SV2)       │  │  (API)       │            │
│  └──────────────┘  └──────────────┘  └──────────────┘            │
└────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **ChannelManager** | Central state for all mining channels, templates, jobs | `Arc<Mutex<ChannelManagerData>>` with message handlers |
| **HTTP API Server** | Accept coinbase update requests, trigger state updates | Axum router with `State(Arc<ChannelManager>)` |
| **Template Receiver** | Fetch templates from Bitcoin Core, forward to ChannelManager | Dedicated thread (BitcoinCoreSv2) or tokio task (SV2 TP) |
| **Mining Message Handler** | Process share submissions, validate, send webhooks | Method on ChannelManager called from downstream I/O tasks |
| **Downstream Connections** | Per-miner connection state, frame I/O | `Downstream` struct with reader/writer tasks |
| **Webhook Client** | POST share notifications to external endpoint | reqwest async client, fire-and-forget with timeout |

## Recommended Project Structure

Existing structure remains. New additions:

```
pool-apps/pool/src/
├── lib/
│   ├── mod.rs                        # Add HTTP API server spawn logic
│   ├── channel_manager/
│   │   ├── mod.rs                    # Add update_coinbase_and_refresh_jobs() method
│   │   ├── mining_message_handler.rs # Add webhook notification on share validation
│   │   └── job_mapping.rs            # NEW: Job-to-user mapping management
│   ├── api/                          # NEW: HTTP API module
│   │   ├── mod.rs                    # API server initialization
│   │   ├── handlers.rs               # Endpoint handlers (update_coinbase)
│   │   └── types.rs                  # Request/response types
│   └── webhooks/                     # NEW: Webhook client module
│       ├── mod.rs                    # Webhook client initialization
│       └── client.rs                 # reqwest wrapper with retry logic
├── main.rs                           # (Unchanged - delegates to lib)
```

### Structure Rationale

- **api/**: Isolated HTTP API logic, follows existing monitoring module pattern
- **webhooks/**: Separate module for outbound HTTP, reusable and testable
- **job_mapping.rs**: Encapsulates job-to-user mapping logic to prevent ChannelManagerData bloat
- **Modifications in-place**: update_coinbase_and_refresh_jobs() added to ChannelManager, webhook calls added to mining_message_handler.rs

## Architectural Patterns

### Pattern 1: Axum State Sharing with Arc<ChannelManager>

**What:** Pass `Arc<ChannelManager>` as Axum application state, extract in handlers via `State()`

**When to use:** When HTTP handlers need read/write access to shared state managed by another component

**Trade-offs:**
- ✅ Simple, idiomatic Axum pattern
- ✅ No additional channels needed for API → ChannelManager communication
- ✅ Handlers can directly call ChannelManager methods
- ⚠️ Lock contention risk if API calls hold locks too long
- ⚠️ Requires ChannelManager methods to be non-blocking or fast

**Example:**
```rust
// In lib/mod.rs (PoolSv2::start)
let channel_manager = ChannelManager::new(...).await?;

// Spawn HTTP API server
let api_server = ApiServer::new(
    self.config.api_bind_address(),
    Arc::new(channel_manager.clone()),
);
task_manager.spawn(async move {
    api_server.run(shutdown_signal).await
});

// In api/handlers.rs
async fn handle_update_coinbase(
    State(channel_manager): State<Arc<ChannelManager>>,
    Json(request): Json<UpdateCoinbaseRequest>,
) -> Result<Json<UpdateCoinbaseResponse>, ApiError> {
    // Validate request
    let address = bitcoin::Address::from_str(&request.address)?;
    let user_id = validate_user_id(request.user_id)?;

    // Call ChannelManager method (acquires lock internally)
    channel_manager.update_coinbase_and_refresh_jobs(address, user_id).await?;

    Ok(Json(UpdateCoinbaseResponse { success: true }))
}
```

### Pattern 2: Fire-and-Forget Webhooks with Timeout

**What:** Spawn tokio task for webhook HTTP POST, set aggressive timeout, log errors but don't block

**When to use:** When external notification must not delay critical path (share validation)

**Trade-offs:**
- ✅ Zero impact on share validation latency
- ✅ Pool continues if webhook endpoint is down
- ⚠️ No delivery guarantee (acceptable for MVP - webhooks are best-effort)
- ⚠️ Can spawn many tasks if shares are frequent (mitigate with task throttling later)

**Example:**
```rust
// In channel_manager/mining_message_handler.rs (after share validation)
pub fn handle_submit_shares_standard(
    &self,
    message: SubmitSharesStandard,
) -> Result<SubmitSharesSuccess, PoolError<Self>> {
    // ... existing validation logic ...

    // Share is valid - send webhook notification
    if let Some(webhook_client) = &self.webhook_client {
        let user_id = self.get_user_id_for_job(message.job_id)?;
        let notification = ShareNotification {
            user_id,
            job_id: message.job_id,
            share_hash: compute_share_hash(&message),
            difficulty: self.current_target_difficulty(),
            timestamp: SystemTime::now(),
            is_block: false, // Updated if share meets block target
        };

        // Fire and forget - spawn task with timeout
        tokio::spawn(async move {
            if let Err(e) = webhook_client.notify(notification).await {
                warn!("Webhook notification failed: {}", e);
            }
        });
    }

    // Continue share processing immediately
    Ok(SubmitSharesSuccess { ...})
}
```

### Pattern 3: Lock-Minimizing State Updates

**What:** Acquire lock, clone required data, drop lock, perform computation, acquire lock again for write

**When to use:** When expensive operations (job recreation, merkle path computation) must not block concurrent access

**Trade-offs:**
- ✅ Minimizes lock hold time
- ✅ Prevents API calls from blocking template updates
- ⚠️ Slightly more complex code (two lock acquisitions)
- ⚠️ State could change between read and write (mitigated by checking consistency)

**Example:**
```rust
pub async fn update_coinbase_and_refresh_jobs(
    &self,
    new_address: bitcoin::Address,
    user_id: UserId,
) -> Result<(), PoolError<ChannelManager>> {
    // Phase 1: Read - acquire lock, clone template, drop lock
    let (template, prev_hash, coinbase_outs) = self.channel_manager_data.super_safe_lock(|data| {
        let template = data.last_future_template
            .as_ref()
            .ok_or(PoolError::NoTemplateAvailable)?
            .clone();
        let prev_hash = data.last_new_prev_hash
            .as_ref()
            .ok_or(PoolError::NoPrevHashAvailable)?
            .clone();
        Ok((template, prev_hash, build_coinbase_outputs(&new_address)))
    })?;

    // Phase 2: Compute - no lock held (CPU-intensive work)
    let new_jobs = recreate_jobs_from_template(
        &template,
        &prev_hash,
        &coinbase_outs,
        &self.pool_tag_string,
    )?;

    // Phase 3: Write - acquire lock, update state, drop lock
    self.channel_manager_data.super_safe_lock(|data| {
        // Consistency check: ensure template hasn't changed
        if data.last_future_template.as_ref() != Some(&template) {
            return Err(PoolError::TemplateChanged);
        }

        data.coinbase_outputs = coinbase_outs;
        data.add_job_to_user_mapping(new_jobs[0].job_id, user_id);

        // Send new jobs to all connected downstreams
        for downstream in data.downstream.values() {
            downstream.send_new_mining_jobs(&new_jobs)?;
        }

        Ok(())
    })?;

    Ok(())
}
```

### Pattern 4: Job-to-User Mapping with LRU Eviction

**What:** HashMap tracking which user_id corresponds to each job_id, cleaned up on SetNewPrevHash

**When to use:** When share submissions arrive with job_id but webhook needs user_id

**Trade-offs:**
- ✅ Fast O(1) lookup during share validation
- ✅ Bounded memory (cleaned on prev_hash change)
- ✅ Handles lagging shares (submitted after coinbase switch)
- ⚠️ Requires cleanup logic to prevent memory leak

**Example:**
```rust
// In channel_manager/job_mapping.rs
pub struct JobMapping {
    job_to_user: HashMap<JobId, UserId>,
}

impl JobMapping {
    pub fn add(&mut self, job_id: JobId, user_id: UserId) {
        self.job_to_user.insert(job_id, user_id);
    }

    pub fn get(&self, job_id: JobId) -> Option<UserId> {
        self.job_to_user.get(&job_id).copied()
    }

    pub fn cleanup_stale_jobs(&mut self, current_prev_hash: &[u8]) {
        // Jobs from previous block are stale - remove mappings
        // (Simplified - actual implementation checks job.prev_hash)
        self.job_to_user.retain(|job_id, _| {
            is_job_current(*job_id, current_prev_hash)
        });
    }
}

// In ChannelManagerData
pub struct ChannelManagerData {
    // ... existing fields ...
    job_mapping: JobMapping,
}

// Called in template_distribution_message_handler when SetNewPrevHash arrives
pub fn handle_set_new_prev_hash(&mut self, message: SetNewPrevHash) -> Result<...> {
    self.channel_manager_data.super_safe_lock(|data| {
        data.last_new_prev_hash = Some(message.clone());

        // Clean up job mappings for old block
        data.job_mapping.cleanup_stale_jobs(&message.prev_hash);
    });

    // ... rest of handler ...
}
```

## Data Flow

### API Call Flow (Update Coinbase)

```
[HTTP POST /update-coinbase]
    ↓
[Axum handler extracts State(Arc<ChannelManager>)]
    ↓
[Validate address format, user_id constraints]
    ↓
[ChannelManager.update_coinbase_and_refresh_jobs(address, user_id)]
    ↓
[Phase 1: Lock → Read template + prev_hash → Unlock]
    ↓
[Phase 2: Build new coinbase outputs, recreate jobs (CPU-bound, no lock)]
    ↓
[Phase 3: Lock → Update coinbase_outputs, add job mapping → Send to downstreams → Unlock]
    ↓
[Broadcast new jobs via downstream_sender channel]
    ↓
[Each Downstream I/O task writes NewMiningJob to miner socket]
    ↓
[HTTP Response 200 OK]
```

**Critical sections requiring lock:**
- Phase 1 read: ~1-5 μs (just HashMap lookups and clones)
- Phase 3 write: ~10-50 μs (HashMap updates + channel sends, no I/O)

**Non-critical sections (no lock held):**
- Phase 2 computation: ~50-200 μs (merkle path, job construction)
- Network I/O: unbounded (async, separate tasks)

### Share Submission Flow (With Webhook)

```
[Miner submits share via Noise TCP]
    ↓
[Downstream I/O reader task receives frame]
    ↓
[Parse SubmitSharesStandard message]
    ↓
[Send message to ChannelManager via downstream_receiver channel]
    ↓
[ChannelManager.handle_submit_shares_standard()]
    ↓
[Lock → Validate share against job, check difficulty → Unlock]
    ↓ (if valid)
[Look up user_id from job_mapping]
    ↓
[Spawn webhook task (fire-and-forget)]
    |    ↓
    |   [reqwest POST to webhook endpoint with 5s timeout]
    |    ↓
    |   [Log error if fails, otherwise discard]
    ↓
[Return SubmitSharesSuccess to Downstream I/O writer task]
    ↓
[Downstream writes success response to miner socket]
```

**Critical path:** Share validation (lock held)
**Non-critical path:** Webhook notification (async task, no blocking)

### Template Update Flow (Concurrent with API Calls)

```
[Bitcoin Core sends NewTemplate via IPC]
    ↓
[Template Receiver task receives template]
    ↓
[Send NewTemplate to ChannelManager via tp_sender channel]
    ↓
[ChannelManager.handle_new_template()]
    ↓
[Lock → Store last_future_template → Unlock]
    ↓
[Recreate jobs with CURRENT coinbase_outputs]
    ↓
[Broadcast new jobs to all downstreams]
```

**Concurrency note:** If API call updates coinbase_outputs while template update is in progress:
- Template update uses old coinbase_outputs (jobs sent to miners)
- Subsequent API call detects template mismatch, re-sends jobs with new coinbase
- No inconsistency: miners just receive two job updates in quick succession

## Integration Points

### HTTP API → ChannelManager

| Method | Parameters | Lock Strategy | Latency |
|--------|------------|---------------|---------|
| `update_coinbase_and_refresh_jobs()` | `Address`, `UserId` | Lock-minimizing (3 phases) | ~100-300 μs |
| Return type: `Result<(), PoolError<ChannelManager>>` | | | |

**Lock ordering:** HTTP API always acquires ChannelManager lock (no deadlock risk - single lock)

### ChannelManager → Webhook Client

| Method | Parameters | Execution | Latency |
|--------|------------|-----------|---------|
| `notify()` | `ShareNotification` | Fire-and-forget (tokio::spawn) | Non-blocking |
| Timeout: 5 seconds | | | |
| Retry: None (best-effort for MVP) | | | |

**Error handling:** Log and discard webhook failures (pool continues)

### Template Receiver → ChannelManager

| Message | Lock Strategy | Frequency |
|---------|---------------|-----------|
| `NewTemplate` | Brief lock to store template, unlock, recreate jobs, lock to broadcast | ~30s (Bitcoin Core getblocktemplate) |
| `SetNewPrevHash` | Brief lock to store prev_hash, cleanup job_mapping | ~10 min (new block found) |

**Concurrency safety:** Template updates and API calls can interleave - API call checks template consistency before applying

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 0-100 miners | Current architecture sufficient. Single ChannelManager instance handles all. |
| 100-1000 miners | Monitor lock contention. If update_coinbase_and_refresh_jobs() blocks template updates >10ms, consider read-write lock (RwLock). |
| 1000+ miners | Lock contention becomes bottleneck. Options: (1) Shard ChannelManager by downstream_id, (2) Eventual consistency model with per-downstream state, (3) Lock-free data structures (crossbeam-skiplist). Not needed for MVP. |

### Scaling Priorities

1. **First bottleneck:** Webhook task spawning under high share rate (1000+ shares/sec)
   - **Mitigation:** Add webhook batching (collect shares for 100ms, send batch)
   - **Detection:** Monitor tokio task count, CPU usage

2. **Second bottleneck:** Lock contention on ChannelManagerData under concurrent API calls + template updates
   - **Mitigation:** Replace `Mutex` with `RwLock` (many readers, single writer)
   - **Detection:** Measure lock hold time with tracing spans

3. **Third bottleneck:** Downstream broadcast channel capacity (bounded at 10)
   - **Mitigation:** Increase capacity or switch to unbounded (memory trade-off)
   - **Detection:** Channel receiver lag errors in logs

## Anti-Patterns

### Anti-Pattern 1: Holding Lock During HTTP I/O

**What people do:** Acquire ChannelManager lock, make HTTP webhook call inside lock, then release
**Why it's wrong:** HTTP I/O can take seconds (if endpoint is slow/down), blocking all other operations (template updates, share validation, other API calls)
**Do this instead:** Clone data needed for webhook, release lock, then make HTTP call in separate task

**Bad:**
```rust
self.channel_manager_data.super_safe_lock(|data| {
    // Validate share...

    // WRONG: HTTP I/O inside lock
    let webhook_url = &data.config.webhook_url;
    reqwest::post(webhook_url)
        .json(&notification)
        .send()
        .await?; // Blocks all other lock acquisitions!

    Ok(())
})
```

**Good:**
```rust
self.channel_manager_data.super_safe_lock(|data| {
    // Validate share, get user_id
    let user_id = data.job_mapping.get(job_id)?;
    Ok(user_id)
})?;

// Lock released - now spawn webhook task
let webhook_client = self.webhook_client.clone();
tokio::spawn(async move {
    webhook_client.notify(notification).await.ok();
});
```

### Anti-Pattern 2: Synchronous Channel for API → ChannelManager

**What people do:** Create `mpsc::channel` between HTTP handlers and ChannelManager, send update requests as messages, wait for response
**Why it's wrong:** Adds unnecessary latency (message passing overhead), complicates error handling (need Result channel), no benefit over direct method call
**Do this instead:** Pass `Arc<ChannelManager>` as Axum state, call methods directly

**Bad:**
```rust
// In API handler
let (tx, rx) = oneshot::channel();
api_to_cm_sender.send(UpdateCoinbaseRequest { address, tx }).await?;
let result = rx.await?; // Extra latency + complexity
```

**Good:**
```rust
// In API handler
State(channel_manager): State<Arc<ChannelManager>>
channel_manager.update_coinbase_and_refresh_jobs(address, user_id).await?;
```

### Anti-Pattern 3: Blocking reqwest Client in Async Context

**What people do:** Use `reqwest::blocking::Client` for webhooks because "it's simpler"
**Why it's wrong:** Blocks tokio worker thread, degrades concurrency, can cause thread exhaustion under load
**Do this instead:** Use async `reqwest::Client`, spawn task if fire-and-forget

**Bad:**
```rust
// In share validation handler (async context)
let client = reqwest::blocking::Client::new(); // WRONG
client.post(webhook_url).json(&notification).send()?; // Blocks thread
```

**Good:**
```rust
// In webhook client module
let client = reqwest::Client::new(); // Async client
tokio::spawn(async move {
    client.post(webhook_url)
        .json(&notification)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok();
});
```

### Anti-Pattern 4: Not Cleaning Up Job-to-User Mappings

**What people do:** Add job_id → user_id mappings on every coinbase update, never remove old entries
**Why it's wrong:** Memory leak - HashMap grows unbounded as jobs accumulate
**Do this instead:** Clean up mappings on `SetNewPrevHash` (jobs from previous block are stale)

**Bad:**
```rust
pub fn handle_set_new_prev_hash(&mut self, message: SetNewPrevHash) {
    self.last_new_prev_hash = Some(message);
    // Forgot to clean up job_mapping - memory leak!
}
```

**Good:**
```rust
pub fn handle_set_new_prev_hash(&mut self, message: SetNewPrevHash) {
    self.last_new_prev_hash = Some(message.clone());

    // Clean up stale job mappings from previous block
    self.job_mapping.cleanup_stale_jobs(&message.prev_hash);
}
```

## Lock Contention Analysis

### Existing Lock Acquisition Points

| Component | Frequency | Hold Time |
|-----------|-----------|-----------|
| Template Receiver (NewTemplate) | Every ~30s | ~5-10 μs (store template) |
| Template Receiver (SetNewPrevHash) | Every ~10 min | ~5-10 μs (store prev_hash) |
| Mining Message Handler (share submit) | 1-1000/sec | ~2-5 μs (validate share) |
| Downstream Connection (setup) | 1-100/hour | ~10-20 μs (add downstream to map) |

### New Lock Acquisition Points (HTTP API)

| Operation | Frequency | Hold Time | Risk Level |
|-----------|-----------|-----------|------------|
| API: update_coinbase (Phase 1 read) | 0.1-10/sec | ~1-5 μs | Low |
| API: update_coinbase (Phase 3 write) | 0.1-10/sec | ~10-50 μs | Low-Medium |

**Total lock time per API call:** ~11-55 μs (two acquisitions)
**Contention risk:** Low for MVP (API calls are infrequent relative to share submissions)

### Deadlock Risk Assessment

**Current architecture:** Single `Mutex<ChannelManagerData>` - no deadlock possible (no lock ordering to violate)

**If adding more locks:** Avoid introducing second lock that could be acquired in reverse order
- ✅ Safe: HTTP API acquires ChannelManager lock only
- ✅ Safe: Webhook client is lock-free (fire-and-forget)
- ⚠️ Risky: Adding Downstream-level lock that's acquired while holding ChannelManager lock

**Recommendation:** Keep single-lock design for MVP. If lock contention becomes issue, upgrade to `RwLock` before adding more locks.

## Webhook Reliability Considerations

### Best-Effort Delivery (MVP)

- **Strategy:** Single POST with 5s timeout, log errors, no retry
- **Acceptable for:** Learning phase, regtest, low-stakes testing
- **Not acceptable for:** Production payout systems

### Production Hardening (Future)

If webhooks become critical for payout attribution:

1. **Persistent Queue:** Write notifications to disk queue (e.g., SQLite), retry from queue on failure
2. **Exponential Backoff:** Retry failed webhooks with delays (1s, 2s, 4s, 8s, max 60s)
3. **Dead Letter Queue:** After N retries, move to DLQ for manual investigation
4. **Idempotency:** Include `notification_id` so webhook endpoint can deduplicate
5. **Monitoring:** Alert if webhook failure rate >5%

**For this milestone:** Best-effort is sufficient. Document webhook unreliability in API docs.

## Recommended Build Order

Phase structure for incremental implementation:

### Phase 1: API Server Skeleton (No State Mutation)

**Goal:** Prove HTTP server integrates with tokio runtime
- Add `axum` dependency to pool Cargo.toml
- Create `api/` module with basic server
- Add `/health` endpoint
- Spawn API server task in `PoolSv2::start()`
- Pass shutdown signal to API server
- **Validation:** curl http://localhost:8080/health returns 200

### Phase 2: Update Coinbase Method (No Jobs)

**Goal:** Prove lock strategy works for state updates
- Add `update_coinbase_outputs()` method to ChannelManager
- Method acquires lock, updates `coinbase_outputs` field, releases lock
- Add POST `/update-coinbase` endpoint that calls method
- **Validation:** curl -X POST with valid address → coinbase_outputs changes in state (verify with debug print or monitoring endpoint)

### Phase 3: Job Recreation and Distribution

**Goal:** Prove jobs can be recreated from stored template
- Add job recreation logic to `update_coinbase_and_refresh_jobs()`
- Use lock-minimizing pattern (3 phases: read, compute, write)
- Broadcast new jobs to downstreams via existing `downstream_sender` channel
- **Validation:** Connected mining-device receives new job within 100ms of API call

### Phase 4: Job-to-User Mapping

**Goal:** Track which user_id owns which job_id
- Add `job_mapping: JobMapping` to ChannelManagerData
- Update `update_coinbase_and_refresh_jobs()` to add mapping
- Add cleanup logic to `handle_set_new_prev_hash()`
- **Validation:** After coinbase switch, job_mapping contains new job_id → user_id entry

### Phase 5: Webhook Notification

**Goal:** Send share notifications to external endpoint
- Add `reqwest` dependency
- Create `webhooks/` module with fire-and-forget client
- Add webhook call to `handle_submit_shares_standard()` and `handle_submit_shares_extended()`
- Configure webhook URL in config.toml
- **Validation:** When share is submitted, webhook endpoint receives POST with correct payload

### Phase 6: Integration Testing and Refinement

**Goal:** Validate end-to-end flow under load
- Test concurrent API calls + template updates
- Test lagging shares (submitted after coinbase switch)
- Test webhook failures (endpoint returns 500) don't break pool
- Measure latency from API call to miner receiving job
- **Validation:** All integration tests pass, latency <100ms

## Sources

**Architecture patterns:**
- Existing codebase analysis: `/home/dan/worktrees/personal/stratum/sv2-apps/dynamic-gsd/.planning/codebase/ARCHITECTURE.md`
- Axum state sharing: `stratum-apps/src/monitoring/http_server.rs` (lines 86-217)
- Lock strategy: `stratum-apps/src/custom_mutex.rs` (super_safe_lock pattern)

**Async HTTP client patterns:**
- Fire-and-forget tasks: Standard tokio pattern (no specific source needed - idiomatic)
- Webhook reliability: General distributed systems best practices (not SV2-specific)

**Confidence notes:**
- HIGH confidence on Axum integration (proven pattern in existing codebase)
- HIGH confidence on lock strategy (super_safe_lock already used throughout)
- MEDIUM confidence on job-to-user mapping cleanup logic (requires validation that job lifecycle assumptions are correct)
- MEDIUM confidence on 100ms latency target (depends on job recreation performance - needs benchmarking)

---
*Architecture research for: HTTP API integration with SV2 pool*
*Researched: 2026-02-05*
