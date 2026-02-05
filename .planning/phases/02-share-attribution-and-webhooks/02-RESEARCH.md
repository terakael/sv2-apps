# Phase 2: Share Attribution and Webhooks - Research

**Researched:** 2026-02-05
**Domain:** Webhook notifications and job-to-user attribution for time-shared mining
**Confidence:** HIGH

## Summary

Phase 2 implements share attribution webhooks that notify external systems when miners submit valid shares or find blocks. The core challenge is tracking which user "owns" each job_id so lagging shares (submitted after coinbase switches) are attributed correctly. This requires maintaining a job_id → user_id mapping that persists across coinbase switches but cleans up stale entries to prevent memory leaks.

The research reveals three established patterns: (1) reqwest is the standard HTTP client for fire-and-forget webhooks in async Rust, providing connection pooling and built-in retry primitives; (2) tokio::spawn enables non-blocking webhook delivery with proper error logging via tracing; (3) HashMap cleanup on SetNewPrevHash events prevents unbounded memory growth by removing jobs from previous blockchain states.

The implementation integrates at share validation points (lines 560, 580, 742, 761 in mining_message_handler.rs), capturing ShareValidationResult::Valid and ShareValidationResult::BlockFound. Job mapping occurs during update_coinbase_and_broadcast (Phase 1) and is consumed when shares arrive. The SetNewPrevHash handler (template_distribution_message_handler.rs:157-240) provides the cleanup hook when Bitcoin Core signals new block arrival.

**Primary recommendation:** Use reqwest::Client with tokio::spawn for fire-and-forget webhooks, track job_id → user_id in ChannelManagerData HashMap, clean up mappings on SetNewPrevHash events to bound memory usage.

## Standard Stack

The established libraries for webhook notifications and job tracking in async Rust:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.12+ | HTTP client for webhooks | Most popular Rust HTTP client, built-in connection pooling, designed for fire-and-forget patterns |
| tokio | 1.44.1 | Async runtime for spawning webhook tasks | Already integrated, tokio::spawn enables non-blocking background tasks |
| serde_json | 1.0 | JSON serialization for webhook payloads | Already integrated, standard for Rust JSON handling |
| tracing | 0.1 | Structured logging for webhook failures | Already integrated, maintains causality in async contexts |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| backoff | 0.4+ | Exponential retry with tokio integration | Optional: Simple retry logic for webhook failures (WEBHOOK-09) |
| chrono | 0.4 | Timestamp generation for webhook payloads | Standard for datetime handling in Rust |

### Already Integrated
| Component | Location | Purpose |
|-----------|----------|---------|
| HashMap<K, V> | std::collections | Job-to-user mapping storage |
| custom_mutex::Mutex | stratum-apps/src/custom_mutex.rs | Thread-safe access to job mappings |
| mining_message_handler | pool/src/lib/channel_manager/mining_message_handler.rs | Share validation integration point (lines 560, 580, 742, 761) |
| template_distribution_message_handler | pool/src/lib/channel_manager/template_distribution_message_handler.rs | SetNewPrevHash cleanup hook (lines 157-240) |

**Installation:**
```bash
# Add to pool-apps/pool/Cargo.toml
reqwest = { version = "0.12", features = ["json"] }
chrono = "0.4"
# backoff = { version = "0.4", features = ["tokio"] }  # Optional for retry
```

## Architecture Patterns

### Recommended Project Structure
```
pool-apps/pool/src/lib/
├── channel_manager/
│   ├── mod.rs                                    # Add job_to_user HashMap to ChannelManagerData
│   ├── mining_message_handler.rs                 # Add webhook calls in share validation (lines 560, 580, 742, 761)
│   └── template_distribution_message_handler.rs  # Add cleanup logic in handle_set_new_prev_hash (line 165)
├── webhook/                                       # NEW: Webhook module
│   ├── mod.rs                                    # WebhookClient with reqwest::Client
│   ├── types.rs                                  # ShareWebhookPayload struct
│   └── client.rs                                 # send_share_notification with tokio::spawn
└── config.rs                                     # Add webhook_url to PoolConfig
```

### Pattern 1: Job-to-User Mapping in ChannelManagerData
**What:** Track job_id → user_id mapping in shared state, populated during coinbase updates, consumed during share validation
**When to use:** Any system requiring attribution across asynchronous events (coinbase switch → share submission)
**Example:**
```rust
// Source: Derived from ChannelManagerData pattern (mod.rs lines 61-82)
use std::collections::HashMap;

pub struct ChannelManagerData {
    // ... existing fields ...

    /// Maps job_id → user_id for share attribution
    /// Populated when update_coinbase_and_broadcast creates jobs
    /// Consumed when handle_submit_shares validates shares
    /// Cleaned up on SetNewPrevHash to prevent memory leak (CONC-03)
    job_to_user: HashMap<u32, String>,
}

impl ChannelManager {
    pub async fn update_coinbase_and_broadcast(
        &self,
        new_address: &str,
        user_id: String,
    ) -> PoolResult<(), error::ChannelManager> {
        // ... validation and job creation ...

        // After generating jobs, store mapping
        self.channel_manager_data.super_safe_lock(|data| {
            // For each job_id created, store the user_id
            // Job IDs come from standard_channel.get_active_job().get_job_id()
            for job_id in created_job_ids {
                data.job_to_user.insert(job_id, user_id.clone());
            }
        });

        Ok(())
    }
}
```

### Pattern 2: Fire-and-Forget Webhook with tokio::spawn
**What:** Non-blocking webhook delivery that doesn't slow share validation, logs errors without crashing pool
**When to use:** Any HTTP notification that must not block critical path operations
**Example:**
```rust
// Source: Derived from tokio spawn patterns and tracing best practices
use reqwest::Client;
use tokio::spawn;
use tracing::{error, info, instrument};

#[derive(Clone)]
pub struct WebhookClient {
    client: Client,
    webhook_url: String,
}

impl WebhookClient {
    pub fn new(webhook_url: String) -> Self {
        Self {
            client: Client::new(),  // Reuse client for connection pooling
            webhook_url,
        }
    }

    /// Sends webhook in background, doesn't block caller
    /// Logs errors but doesn't propagate them (WEBHOOK-09)
    #[instrument(skip(self, payload), fields(user_id = %payload.user_id, job_id = payload.job_id))]
    pub fn send_share_notification(&self, payload: ShareWebhookPayload) {
        let client = self.client.clone();
        let url = self.webhook_url.clone();

        // Spawn background task (fire-and-forget pattern)
        spawn(async move {
            match client.post(&url)
                .json(&payload)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Webhook delivered successfully");
                    } else {
                        error!(
                            status = %response.status(),
                            "Webhook delivery failed with non-2xx status"
                        );
                    }
                }
                Err(e) => {
                    error!(error = %e, "Webhook delivery failed");
                }
            }
        });

        // Return immediately, don't wait for webhook
    }
}
```

### Pattern 3: SetNewPrevHash Cleanup Hook
**What:** Clean up stale job mappings when blockchain advances, preventing unbounded memory growth
**When to use:** Any mapping tied to blockchain state that needs garbage collection
**Example:**
```rust
// Source: template_distribution_message_handler.rs handle_set_new_prev_hash (lines 157-240)
async fn handle_set_new_prev_hash(
    &mut self,
    _server_id: Option<usize>,
    msg: SetNewPrevHash<'_>,
    _tlv_fields: Option<&[Tlv]>,
) -> Result<(), Self::Error> {
    info!("Received: {}", msg);

    // Clean up stale job mappings BEFORE processing new prevhash
    self.channel_manager_data.super_safe_lock(|data| {
        let jobs_before = data.job_to_user.len();

        // Retain only jobs that are still valid
        // Jobs from previous templates become stale when new prevhash arrives
        // Strategy: Remove all jobs, they'll be repopulated by next coinbase update
        // Alternative: Track job creation timestamps and remove jobs older than N seconds
        data.job_to_user.clear();

        let jobs_after = data.job_to_user.len();
        info!(
            "Cleaned up {} stale job mappings (before: {}, after: {})",
            jobs_before - jobs_after,
            jobs_before,
            jobs_after
        );
    });

    // Continue with normal SetNewPrevHash processing
    // ... existing logic from lines 165-240 ...

    Ok(())
}
```

### Pattern 4: Share Validation Integration
**What:** Hook webhook calls into existing share validation flow, extract job_id and user_id
**When to use:** Adding observability to existing validation logic without changing core behavior
**Example:**
```rust
// Source: mining_message_handler.rs handle_submit_shares_standard (lines 519-679)
async fn handle_submit_shares_standard(
    &mut self,
    client_id: Option<usize>,
    msg: SubmitSharesStandard,
    _tlv_fields: Option<&[Tlv]>,
) -> Result<(), Self::Error> {
    let downstream_id = client_id.expect("client_id must be present");

    let messages = self.channel_manager_data.super_safe_lock(|channel_manager_data| {
        let Some(downstream) = channel_manager_data.downstream.get(&downstream_id) else {
            return Err(PoolError::disconnect(...));
        };

        downstream.downstream_data.super_safe_lock(|downstream_data| {
            let Some(standard_channel) = downstream_data.standard_channels.get_mut(&msg.channel_id) else {
                // ... error handling ...
            };

            let res = standard_channel.validate_share(msg.clone());

            match res {
                Ok(ShareValidationResult::Valid(share_hash)) => {
                    // NEW: Extract user_id from job mapping
                    let user_id = channel_manager_data.job_to_user
                        .get(&msg.job_id)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());

                    // NEW: Send webhook notification (fire-and-forget)
                    let payload = ShareWebhookPayload {
                        user_id,
                        share_hash: share_hash.to_string(),
                        difficulty: standard_channel.get_target().difficulty_float(),
                        job_id: msg.job_id,
                        channel_id: msg.channel_id,
                        downstream_id,
                        sequence_number: msg.sequence_number,
                        timestamp: chrono::Utc::now().timestamp(),
                        is_block: false,
                    };
                    self.webhook_client.send_share_notification(payload);

                    // ... existing success handling ...
                }
                Ok(ShareValidationResult::BlockFound(share_hash, template_id, coinbase)) => {
                    // NEW: Extract user_id and send block webhook
                    let user_id = channel_manager_data.job_to_user
                        .get(&msg.job_id)
                        .cloned()
                        .unwrap_or_else(|| "unknown".to_string());

                    let payload = ShareWebhookPayload {
                        user_id,
                        share_hash: share_hash.to_string(),
                        difficulty: standard_channel.get_target().difficulty_float(),
                        job_id: msg.job_id,
                        channel_id: msg.channel_id,
                        downstream_id,
                        sequence_number: msg.sequence_number,
                        timestamp: chrono::Utc::now().timestamp(),
                        is_block: true,  // Flag block solutions
                    };
                    self.webhook_client.send_share_notification(payload);

                    // ... existing block found handling ...
                }
                Err(ShareValidationError::InvalidJobId) => {
                    // NOTE: Invalid job_id means job not found in channel
                    // This could be a lagging share from previous template
                    // Check job_to_user mapping for attribution before error
                    let user_id = channel_manager_data.job_to_user.get(&msg.job_id);
                    if let Some(user_id) = user_id {
                        info!(
                            "Lagging share from previous template: job_id={}, user_id={}",
                            msg.job_id,
                            user_id
                        );
                    }
                    // ... existing error handling ...
                }
                // ... other validation errors ...
            }

            Ok(messages)
        })
    })?;

    Ok(())
}
```

### Anti-Patterns to Avoid

- **Awaiting webhook in critical path**: Never `.await` webhook delivery in share validation - blocks validation for all miners. Always use tokio::spawn for fire-and-forget.
- **Unbounded HashMap growth**: Never store job mappings without cleanup - memory leak. Always clean up on SetNewPrevHash or implement time-based expiry.
- **Panicking in spawned tasks**: Never unwrap() in webhook tasks - poison doesn't propagate, just silently fails. Always use proper error handling with tracing.
- **Sharing reqwest::Client per request**: Never create new Client for each webhook - connection overhead. Always reuse single Client instance for connection pooling.

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client for webhooks | Custom hyper client with manual connection pooling | reqwest::Client with connection pooling built-in | Handles connection reuse, timeouts, redirects, TLS out of the box |
| Exponential backoff retry | Custom retry loop with sleep and multiplier | backoff crate with tokio integration | Handles randomization (jitter), max elapsed time, permanent vs transient errors |
| Timestamp generation | Manual SystemTime arithmetic | chrono::Utc::now().timestamp() | Handles timezone conversion, formatting, arithmetic correctly |
| JSON serialization | Manual string concatenation | serde_json with #[derive(Serialize)] | Type-safe, handles escaping, nested structures, arrays automatically |
| Background task spawning | Custom thread pools or futures::executor | tokio::spawn for lightweight tasks | Already integrated, ~64 bytes per task, proper cancellation handling |

**Key insight:** reqwest is built on hyper but adds convenience layers that prevent common mistakes (connection reuse, timeout handling, redirect policies). Use reqwest unless you need hyper's low-level control.

## Common Pitfalls

### Pitfall 1: Job Mapping Populated Too Late (Attribution Gap)
**What goes wrong:** Job mapping inserted AFTER jobs distributed to miners. Miner submits share before mapping exists. Share attributed to "unknown" user even though it's valid.
**Why it happens:** Natural ordering: create jobs, distribute messages, store mapping. But network is fast - miner can submit share before mapping stored.
**How to avoid:**
- Store job_id → user_id mapping BEFORE calling message.forward() to broadcast jobs
- Use atomic operation: super_safe_lock covers both mapping insert and message generation
- Critical section: data.job_to_user.insert(job_id, user_id) → messages.push(job)
**Warning signs:** "unknown" user_id in webhooks immediately after coinbase switch, integration test failures showing missing attribution
**Evidence:** Common concurrency bug pattern in event-driven systems - see Pattern 4 for correct ordering

### Pitfall 2: Webhook Blocking Share Validation (Performance Degradation)
**What goes wrong:** Webhook .await called in share validation lock. Every share waits for HTTP roundtrip (50-200ms). Pool becomes unresponsive, miners disconnect due to timeout.
**Why it happens:** Natural instinct to wait for webhook confirmation, ensure delivery before continuing. But share validation is in critical path for all miners.
**How to avoid:**
- Always use tokio::spawn for webhook delivery, never .await in validation path
- Return from validation immediately after spawn(), don't wait for JoinHandle
- Accept that webhook delivery is best-effort, not guaranteed
**Warning signs:** Lock hold times spike to 100ms+, share validation latency increases, miners report high reject rates
**Evidence:** WEBHOOK-08 requirement explicitly states "fire-and-forget pattern (non-blocking)"

### Pitfall 3: Memory Leak from Unbounded Job Mapping (Resource Exhaustion)
**What goes wrong:** Job mappings stored but never cleaned up. HashMap grows indefinitely. Pool OOMs after hours/days of operation. Restart required.
**Why it happens:** Jobs are created continuously but no natural expiry point. SetNewPrevHash arrives irregularly (10min average). Easy to forget cleanup.
**How to avoid:**
- Implement cleanup in handle_set_new_prev_hash hook (line 165 in template_distribution_message_handler.rs)
- Strategy 1: Clear all mappings on SetNewPrevHash (simple, works because jobs become stale)
- Strategy 2: Retain jobs from current template only (requires tracking template_id per job)
- Strategy 3: Time-based expiry (remove jobs older than 15 minutes)
**Warning signs:** Gradual memory growth over hours, HashMap.len() monotonically increasing, pool crashes with OOM
**Evidence:** CONC-03 requirement explicitly states "Pool cleans up stale job-to-user mappings on SetNewPrevHash (prevents memory leak)"

### Pitfall 4: Lagging Share Attribution Lost (Incorrect User Credit)
**What goes wrong:** Miner submits share for old job after coinbase switch. Job mapping already deleted. Share attributed to "unknown" or rejected with InvalidJobId.
**Why it happens:** Network latency means shares arrive out of order. Miner working on job N, pool switches to job N+1, miner submits share for N after switch completes.
**How to avoid:**
- Keep job mappings for at least 2 SetNewPrevHash cycles (~20 minutes minimum)
- Don't immediately clear mappings on coinbase switch - only on blockchain state change
- InvalidJobId error should still check job_to_user mapping before failing
**Warning signs:** Spike in InvalidJobId errors after coinbase switches, users report missing share credits, "unknown" user attribution increases
**Evidence:** WEBHOOK-07 requirement explicitly states "Job-to-user mapping handles lagging shares submitted after coinbase switch"

### Pitfall 5: Webhook Failures Silent (No Observability)
**What goes wrong:** Webhooks fail (network error, timeout, 503) but no visibility. External system never receives shares. Debugging impossible without logs.
**Why it happens:** tokio::spawn tasks don't propagate errors to parent. If spawned task panics or returns Err, it's silently dropped unless explicitly logged.
**How to avoid:**
- Always use tracing::error! in spawned webhook tasks for failures
- Use #[instrument] macro to add span context (user_id, job_id) to logs
- Log successful deliveries at info level for audit trail
- Monitor webhook error rate separately from pool operation
**Warning signs:** External system reports missing data, no logs explaining why, silent failures
**Evidence:** WEBHOOK-09 requirement explicitly states "Webhook failures logged but don't block pool operation"

## Code Examples

Verified patterns from official sources:

### Webhook Payload Type
```rust
// Implements WEBHOOK-02, WEBHOOK-03, WEBHOOK-04, WEBHOOK-05
use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct ShareWebhookPayload {
    /// User identifier from coinbase update API call (WEBHOOK-02)
    pub user_id: String,

    /// Share hash in hex format (WEBHOOK-03)
    pub share_hash: String,

    /// Difficulty of the share (WEBHOOK-03)
    pub difficulty: f64,

    /// Job ID that the share was submitted for (WEBHOOK-03)
    pub job_id: u32,

    /// Unix timestamp when share was validated (WEBHOOK-03)
    pub timestamp: i64,

    /// True for BlockFound, false for Valid shares (WEBHOOK-04)
    pub is_block: bool,

    /// Channel ID that submitted the share (WEBHOOK-05)
    pub channel_id: u32,

    /// Downstream ID (mining connection) (WEBHOOK-05)
    pub downstream_id: usize,

    /// Sequence number of the share submission (WEBHOOK-05)
    pub sequence_number: u32,
}
```

### WebhookClient with reqwest
```rust
// Implements WEBHOOK-01, WEBHOOK-08, WEBHOOK-09
use reqwest::Client;
use std::time::Duration;
use tokio::spawn;
use tracing::{error, info, instrument};

#[derive(Clone)]
pub struct WebhookClient {
    client: Client,
    webhook_url: String,
}

impl WebhookClient {
    pub fn new(webhook_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))  // Prevent hanging
            .pool_max_idle_per_host(10)      // Connection reuse
            .build()
            .expect("Failed to create webhook client");

        Self {
            client,
            webhook_url,
        }
    }

    /// Sends webhook notification in background (WEBHOOK-08: fire-and-forget)
    /// Logs errors but doesn't block pool operation (WEBHOOK-09)
    #[instrument(skip(self, payload), fields(
        user_id = %payload.user_id,
        job_id = payload.job_id,
        is_block = payload.is_block
    ))]
    pub fn send_share_notification(&self, payload: ShareWebhookPayload) {
        let client = self.client.clone();
        let url = self.webhook_url.clone();

        // WEBHOOK-08: Fire-and-forget pattern (non-blocking)
        spawn(async move {
            match client.post(&url)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Webhook delivered successfully");
                    } else {
                        // WEBHOOK-09: Log failure but don't block pool
                        error!(
                            status = %response.status(),
                            "Webhook delivery failed with non-2xx status"
                        );
                    }
                }
                Err(e) => {
                    // WEBHOOK-09: Log failure but don't block pool
                    error!(error = %e, "Webhook delivery failed");
                }
            }
        });

        // Return immediately, don't wait for HTTP response
    }
}
```

### Job Mapping in ChannelManagerData
```rust
// Implements WEBHOOK-06
// Source: Derived from ChannelManagerData (mod.rs lines 61-82)
use std::collections::HashMap;

pub struct ChannelManagerData {
    // ... existing fields ...

    /// Maps job_id → user_id for share attribution (WEBHOOK-06)
    /// Populated when update_coinbase_and_broadcast creates jobs
    /// Consumed when handle_submit_shares validates shares
    /// Cleaned up on SetNewPrevHash to prevent memory leak (CONC-03)
    pub(crate) job_to_user: HashMap<u32, String>,
}
```

### Populating Job Mapping During Coinbase Update
```rust
// Implements WEBHOOK-06: Track job_id → user_id mapping
// Source: Derived from update_coinbase_and_broadcast (mod.rs lines 695-876)
pub async fn update_coinbase_and_broadcast(
    &self,
    new_address: &str,
    user_id: String,
) -> PoolResult<(), error::ChannelManager> {
    // ... address validation and job creation ...

    let messages = self.channel_manager_data.super_safe_lock(|channel_manager_data| {
        // Update coinbase outputs
        channel_manager_data.coinbase_outputs = new_encoded_outputs.clone();

        let mut messages: Vec<RouteMessageTo> = Vec::new();

        // Iterate downstreams and generate jobs
        for (downstream_id, downstream) in channel_manager_data.downstream.iter_mut() {
            let downstream_messages = downstream.downstream_data.super_safe_lock(|data| {
                // Create jobs and extract job IDs
                // ... job creation logic ...

                // CRITICAL: Store job mapping BEFORE broadcasting jobs
                // Extract job_id from standard/extended channels
                for (_channel_id, standard_channel) in data.standard_channels.iter() {
                    if let Some(job) = standard_channel.get_active_job() {
                        let job_id = job.get_job_id();
                        channel_manager_data.job_to_user.insert(job_id, user_id.clone());
                    }
                }

                Ok(messages)
            })?;

            messages.extend(downstream_messages);
        }

        Ok(messages)
    })?;

    // Broadcast messages AFTER mappings stored
    for message in messages {
        message.forward(&self.channel_manager_channel).await;
    }

    Ok(())
}
```

### Cleanup on SetNewPrevHash
```rust
// Implements CONC-03: Clean up stale job mappings
// Source: template_distribution_message_handler.rs handle_set_new_prev_hash (lines 157-240)
async fn handle_set_new_prev_hash(
    &mut self,
    _server_id: Option<usize>,
    msg: SetNewPrevHash<'_>,
    _tlv_fields: Option<&[Tlv]>,
) -> Result<(), Self::Error> {
    info!("Received: {}", msg);

    // CONC-03: Clean up stale job mappings to prevent memory leak
    self.channel_manager_data.super_safe_lock(|data| {
        let jobs_before = data.job_to_user.len();

        // Strategy: Clear all mappings on new prevhash
        // Jobs become stale when blockchain advances
        // Next coinbase update will repopulate for new template
        data.job_to_user.clear();

        info!(
            "Cleaned up {} stale job mappings on SetNewPrevHash",
            jobs_before
        );
    });

    // Continue with normal SetNewPrevHash processing
    let messages = self.channel_manager_data.super_safe_lock(|data| {
        data.last_new_prev_hash = Some(msg.clone().into_static());
        // ... existing logic ...
    })?;

    Ok(())
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| hyper manual client | reqwest::Client with connection pooling | reqwest 0.11+ (2021) | Simpler API, automatic connection reuse, built-in retry primitives |
| futures::spawn | tokio::spawn for green threads | tokio 1.0+ (2020) | Lightweight tasks (~64 bytes), better cancellation, structured concurrency |
| Manual HashMap cleanup loops | HashMap::retain for in-place filtering | Rust stable | O(n) cleanup without allocation, cleaner API |
| SystemTime manual arithmetic | chrono for timestamp handling | Established pattern | Timezone-aware, correct leap second handling, human-readable formatting |
| Manual error propagation in spawned tasks | tracing with #[instrument] | tracing 0.1+ | Maintains causality across async boundaries, structured logging |

**Deprecated/outdated:**
- hyper client for simple webhooks: Use reqwest unless you need low-level control
- ThreadPool for background tasks: Use tokio::spawn for async tasks
- Hardcoded webhook URLs: Load from config (PoolConfig) for environment flexibility

## Open Questions

Things that couldn't be fully resolved:

1. **Job ID extraction from channels**
   - What we know: standard_channel.get_active_job() returns job with get_job_id() method
   - What's unclear: Whether job_id is unique across standard/extended channels, or scoped per downstream
   - Recommendation: Assume job_id is globally unique (template provider assigns), verify with integration test

2. **Lagging share time window**
   - What we know: SetNewPrevHash arrives ~10min average (Bitcoin block time)
   - What's unclear: How long to keep job mappings for lagging shares (15min? 20min? 2 blocks?)
   - Recommendation: Start with simple clear-all strategy on SetNewPrevHash, measure InvalidJobId rates, adjust if needed

3. **Webhook endpoint configuration**
   - What we know: Webhook URL should come from PoolConfig
   - What's unclear: Whether to support multiple webhook endpoints (different URLs for shares vs blocks)
   - Recommendation: Single webhook_url in config for MVP, external system can filter by is_block flag

4. **Extended channel share webhooks**
   - What we know: handle_submit_shares_extended has similar structure to standard (lines 681-871)
   - What's unclear: Whether extended channels need separate attribution logic or same pattern applies
   - Recommendation: Implement for both standard and extended using same pattern, test with both channel types

## Sources

### Primary (HIGH confidence)
- https://docs.rs/reqwest/latest/reqwest/ - HTTP client library, connection pooling patterns (verified 2026-02-05)
- https://tokio.rs/tokio/tutorial/spawning - tokio::spawn for fire-and-forget tasks (verified 2026-02-05)
- https://doc.rust-lang.org/std/collections/struct.HashMap.html - HashMap cleanup patterns, retain() method (verified 2026-02-05)
- https://docs.rs/backoff/latest/backoff/ - Exponential backoff retry patterns (verified 2026-02-05)
- pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs - Share validation integration points (lines 560, 580, 742, 761)
- pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs - SetNewPrevHash handler cleanup hook (lines 157-240)
- pool-apps/pool/src/lib/channel_manager/mod.rs - ChannelManagerData structure, update_coinbase_and_broadcast pattern
- .planning/REQUIREMENTS.md - WEBHOOK-* requirements (lines 26-36), CONC-03 (line 42)

### Secondary (MEDIUM confidence)
- https://rust-lang.github.io/async-book/ - Async patterns (join vs spawn distinction)
- https://tokio.rs/tokio/topics/tracing - Error logging in spawned tasks
- stratum-apps/src/rpc/mini_rpc_client.rs - Existing hyper usage for RPC (lines 1-216)
- Job ID uniqueness assumption: Based on template_id scoping in channels-sv2, needs integration test validation

### Tertiary (LOW confidence)
- Lagging share time window: Industry practice (~2 blocks) but not verified for SV2 pools
- Optimal cleanup frequency: Depends on pool scale and block arrival variance

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - reqwest is established standard for HTTP clients, tokio::spawn is native async primitive
- Architecture: HIGH - Patterns derived from existing codebase (mining_message_handler, template_distribution_message_handler)
- Integration points: HIGH - Exact line numbers provided for share validation hooks
- Pitfalls: HIGH - Memory leak prevention (CONC-03) and lagging shares (WEBHOOK-07) are explicit requirements
- Cleanup strategy: MEDIUM - SetNewPrevHash is correct hook, but exact retention policy needs testing

**Research date:** 2026-02-05
**Valid until:** 30 days (stable domain, libraries mature, requirements well-defined)
