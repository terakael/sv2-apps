# Phase 1: API Foundation and Template Switching - Research

**Researched:** 2026-02-05
**Domain:** HTTP API integration with async Rust mining pool
**Confidence:** HIGH

## Summary

Phase 1 establishes the HTTP API foundation for dynamic coinbase switching and validates the core feasibility assumption that merkle paths remain valid when coinbase outputs change. The research reveals this is a straightforward integration using the pool's existing architecture: axum 0.8.7 is already integrated for monitoring endpoints, the bitcoin crate (via miniscript) provides production-ready address validation, and the template distribution handler demonstrates the exact job recreation pattern needed.

The primary technical risk is merkle path validity after coinbase modification. The codebase shows that coinbase_outputs are already modified in template_distribution_message_handler.rs (line 43-44), which suggests the architecture supports this operation. However, this assumption MUST be validated with an explicit test before implementing any other features. If shares are rejected after coinbase changes, the entire approach fails.

The implementation path is well-established: add an HTTP POST endpoint to the existing pool struct, reuse the template recreation logic from handle_new_template, and ensure atomic state updates using the existing custom_mutex pattern. Lock ordering discipline (channel_manager_data before downstream_data) prevents deadlocks. The sub-100ms broadcast requirement is achievable based on the existing monitoring that shows template distribution completes within microseconds under normal conditions.

**Primary recommendation:** Validate merkle path reuse with a test FIRST, then implement HTTP API using axum State pattern, then add job recreation logic reusing handle_new_template patterns.

## Standard Stack

The established libraries for HTTP API integration in async Rust with Bitcoin validation:

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | 0.8.7 | HTTP API server | Already integrated for monitoring, designed for tokio, production-proven State pattern |
| bitcoin | 0.32+ | Address validation | Used via miniscript in CoinbaseRewardScript, comprehensive validation with network checking |
| tokio | 1.44.1 | Async runtime | Already the pool's runtime, powers all I/O and message passing |
| serde_json | 1.0 | JSON parsing | Already integrated, standard for Rust JSON handling |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tower-http | 0.6.8 | HTTP middleware | Timeout, tracing, request logging - production hardening |
| tracing | 0.1 | Structured logging | Already integrated, use for API request/response logging |

### Already Integrated
| Component | Location | Purpose |
|-----------|----------|---------|
| custom_mutex::Mutex | stratum-apps/src/custom_mutex.rs | Safe lock pattern for shared state |
| ChannelManager | pool-apps/pool/src/lib/channel_manager/mod.rs | Central state management |
| monitoring HTTP server | stratum-apps/src/monitoring/http_server.rs | Axum integration example |

**Installation:**
```bash
# Already in Cargo.toml, no new dependencies needed for core functionality
# Only tower-http is new (optional, for production middleware):
tower-http = "0.6.8"
```

## Architecture Patterns

### Recommended Project Structure
```
pool-apps/pool/src/lib/
├── channel_manager/
│   ├── mod.rs                                    # Add HTTP API methods here
│   ├── template_distribution_message_handler.rs  # Pattern to replicate
│   └── mining_message_handler.rs                 # Share validation hooks
├── http_api/                                     # NEW: HTTP API module
│   ├── mod.rs                                    # Router setup
│   ├── handlers.rs                               # POST /api/coinbase handler
│   └── types.rs                                  # Request/response types
└── lib.rs                                        # Start HTTP server in PoolSv2::start()
```

### Pattern 1: Axum HTTP API Integration with Shared State
**What:** HTTP server shares ChannelManager via Arc, handlers use State extractor for type-safe access
**When to use:** Any HTTP API that needs to access pool state
**Example:**
```rust
// Source: stratum-apps/src/monitoring/http_server.rs (lines 15-92)
use axum::{extract::State, Router, routing::post, Json, http::StatusCode};
use std::sync::Arc;

#[derive(Clone)]
struct ApiState {
    channel_manager: ChannelManager,
}

async fn start_http_server(
    channel_manager: ChannelManager,
    bind_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = ApiState { channel_manager };

    let app = Router::new()
        .route("/api/coinbase", post(handle_coinbase_update))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_coinbase_update(
    State(state): State<ApiState>,
    Json(payload): Json<CoinbaseRequest>,
) -> Result<StatusCode, StatusCode> {
    // Handler has typed access to channel_manager via state
    state.channel_manager.update_coinbase(payload.address, payload.user_id).await
        .map(|_| StatusCode::OK)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

### Pattern 2: Bitcoin Address Validation
**What:** Parse and validate Bitcoin addresses using bitcoin crate via miniscript descriptors
**When to use:** Any user-provided Bitcoin address before storing in pool state
**Example:**
```rust
// Source: stratum-apps/src/config_helpers/coinbase_output/mod.rs (lines 22-91)
use miniscript::bitcoin::{Address, Network, address::NetworkUnchecked};

fn validate_bitcoin_address(address_str: &str, network: Network) -> Result<Vec<u8>, AddressError> {
    // Parse address without network check first
    let address: Address<NetworkUnchecked> = address_str.parse()
        .map_err(|_| AddressError::InvalidFormat)?;

    // Validate network compatibility
    if !address.is_valid_for_network(network) {
        return Err(AddressError::WrongNetwork);
    }

    // Convert to scriptPubKey for storage
    let script_pubkey = address.assume_checked_ref().script_pubkey();
    Ok(script_pubkey.to_bytes())
}

// Alternative: Use CoinbaseRewardScript for descriptor parsing
use stratum_apps::config_helpers::CoinbaseRewardScript;

fn validate_via_descriptor(address_str: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Wrap in addr() descriptor format
    let descriptor = format!("addr({})", address_str);
    let reward_script = CoinbaseRewardScript::from_descriptor(&descriptor)?;
    Ok(reward_script.script_pubkey().to_bytes())
}
```

### Pattern 3: Lock-Minimizing State Update (Prevents Deadlock)
**What:** Three-phase update pattern: read state, compute outside lock, acquire lock for write
**When to use:** When API calls and template updates access same shared state concurrently
**Example:**
```rust
// Source: Derived from pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs
// and .planning/research/PITFALLS.md deadlock prevention pattern

async fn update_coinbase_and_broadcast(
    &self,
    new_address: Vec<u8>,
    user_id: String,
) -> Result<(), PoolError<error::ChannelManager>> {
    // Phase 1: Read template state (short lock)
    let template_data = self.channel_manager_data.super_safe_lock(|data| {
        let template = data.last_future_template
            .as_ref()
            .ok_or(PoolError::log("No template available"))?;
        Ok::<_, PoolError<error::ChannelManager>>(template.clone())
    })?;

    // Phase 2: Compute new jobs OUTSIDE lock (no contention)
    let new_coinbase_outputs = vec![/* new outputs with address */];
    let coinbase_output = deserialize_outputs(new_coinbase_outputs.clone())
        .map_err(|e| PoolError::log(e))?;

    // Phase 3: Atomic state update (short lock)
    let messages = self.channel_manager_data.super_safe_lock(|channel_manager_data| {
        // Update coinbase outputs
        channel_manager_data.coinbase_outputs = new_coinbase_outputs;

        let mut messages = Vec::new();

        // Recreate jobs for all downstreams (following handle_new_template pattern)
        for (downstream_id, downstream) in channel_manager_data.downstream.iter_mut() {
            let downstream_messages = downstream.downstream_data.super_safe_lock(|data| {
                // Recreate jobs with new coinbase - pattern from template_distribution_message_handler.rs lines 54-111
                // ... job recreation logic ...
                Ok::<Vec<RouteMessageTo>, PoolError<error::ChannelManager>>(vec![])
            })?;
            messages.extend(downstream_messages);
        }

        Ok::<Vec<RouteMessageTo>, PoolError<error::ChannelManager>>(messages)
    })?;

    // Phase 4: Broadcast outside lock (no contention during I/O)
    for message in messages {
        message.forward(&self.channel_manager_channel).await;
    }

    Ok(())
}
```

### Pattern 4: Job Recreation from Stored Template
**What:** Recreate mining jobs from last_future_template with modified coinbase outputs
**When to use:** When coinbase changes but template from Bitcoin Core hasn't updated
**Example:**
```rust
// Source: pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs (lines 37-134)
// Exact pattern from handle_new_template - shows how to recreate jobs with new coinbase

let messages = self.channel_manager_data.super_safe_lock(|channel_manager_data| {
    // Deserialize current coinbase outputs and modify
    let mut coinbase_output = deserialize_outputs(channel_manager_data.coinbase_outputs.clone())
        .expect("deserialization failed");

    // Optionally adjust value (for template updates) or keep same (for address-only updates)
    // coinbase_output[0].value = Amount::from_sat(msg.coinbase_tx_value_remaining);

    let mut messages: Vec<RouteMessageTo> = Vec::new();

    for (downstream_id, downstream) in channel_manager_data.downstream.iter_mut() {
        let requires_custom_work = downstream.requires_custom_work.load(Ordering::SeqCst);
        if requires_custom_work {
            continue; // Skip job declarator clients
        }

        let messages_: Vec<RouteMessageTo<'_>> = downstream.downstream_data.super_safe_lock(|data| {
            // Update group channel with new coinbase
            // Note: For coinbase-only updates, reuse last_future_template, don't call on_new_template
            // Just recreate jobs from existing template with modified coinbase

            let requires_standard_jobs = downstream.requires_standard_jobs.load(Ordering::SeqCst);
            let mut messages: Vec<RouteMessageTo> = vec![];

            // Standard channels: send NewMiningJob
            for (channel_id, standard_channel) in data.standard_channels.iter_mut() {
                if requires_standard_jobs {
                    // Get active job and send (job already has new coinbase from group channel update)
                    let standard_job = standard_channel.get_active_job().expect("active job must exist");
                    messages.push((*downstream_id, Mining::NewMiningJob(standard_job.get_job_message().clone())).into());
                }
            }

            Ok::<Vec<RouteMessageTo<'_>>, Self::Error>(messages)
        })?;

        messages.extend(messages_);
    }
    Ok::<Vec<RouteMessageTo<'_>>, Self::Error>(messages)
})?;

// Broadcast messages to all downstreams
for message in messages {
    message.forward(&self.channel_manager_channel).await;
}
```

### Anti-Patterns to Avoid
- **Nested lock acquisition**: Never acquire downstream_data lock while holding channel_manager_data lock. Clone downstream collection first (Pitfall 1 from research)
- **Holding locks during I/O**: Never hold mutex while calling HTTP endpoints or writing files. Complete I/O outside lock scope (Performance trap from research)
- **Unwrap inside super_safe_lock**: Never panic inside lock closures - poisons the mutex and deadlocks entire pool (Technical debt pattern from research)

## Don't Hand-Roll

Problems that look simple but have existing solutions:

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Bitcoin address validation | Custom base58/bech32 parser | bitcoin crate + miniscript | Network validation, checksum verification, all formats (P2PKH, P2SH, P2WPKH, P2WSH, P2TR) |
| HTTP JSON API | Manual TCP + JSON parsing | axum with Json extractor | Automatic deserialization, type safety, concurrent request handling |
| Lock ordering discipline | Manual mutex acquisition | custom_mutex with safe_lock pattern | Explicit error handling, poison recovery, closure-based safety |
| Job recreation logic | Custom job builder | Reuse handle_new_template pattern | Already handles all channel types (standard, extended, group), tested in production |

**Key insight:** The pool already has all the patterns needed. Don't reinvent job distribution, lock management, or address validation - follow existing code paths.

## Common Pitfalls

### Pitfall 1: Nested Lock Acquisition (Deadlock)
**What goes wrong:** Acquiring channel_manager_data lock, then downstream_data lock from within same closure creates deadlock. Pool becomes unresponsive, miners disconnect.
**Why it happens:** Natural nesting when iterating downstreams inside channel manager lock. `data.downstreams.get(id).lock()` called within `channel_manager_data.super_safe_lock()` violates lock ordering.
**How to avoid:**
- Clone downstream collection BEFORE acquiring child locks
- Establish strict ordering: channel_manager_data → downstream_data, never reverse
- Keep critical sections short: extract data, release lock, then process
**Warning signs:** Pool hangs under load, tokio-console shows blocked mutex waiters, integration tests hang intermittently
**Evidence:** Fixed in commit 02ea18c7 - see .planning/research/PITFALLS.md lines 36-56

### Pitfall 2: Merkle Path Invalidation on Coinbase Change
**What goes wrong:** Changing coinbase outputs potentially invalidates cached merkle paths. Miners submit shares with correct POW but invalid merkle roots. Pool rejects 100% of shares.
**Why it happens:** Bitcoin merkle path is computed from hash(coinbase) || merkle_path[0] || .... If implementation caches merkle branches from original coinbase, changing coinbase breaks merkle root validation.
**How to avoid:**
- TEST THIS ASSUMPTION FIRST: Create test that switches coinbase, validates share, asserts success
- Understand channels-sv2 architecture: determine if merkle root is recomputed per share or cached
- Codebase shows coinbase_outputs modified in template handler (line 43-44), suggesting architecture supports this
**Warning signs:** All shares rejected after coinbase switch with "Invalid merkle root" errors, miners disconnect immediately
**Evidence:** See .planning/research/PITFALLS.md lines 146-182 - identified as feasibility blocker

### Pitfall 3: Template Provider Race Condition
**What goes wrong:** HTTP API updates coinbase_outputs while template handler reads coinbase_outputs to create jobs. Race causes mismatched jobs, state corruption, or wrong user attribution.
**Why it happens:** Two async tasks (API handler, template handler) access shared mutable state without coordination.
**How to avoid:**
- Atomic coinbase update: regenerate AND redistribute jobs in same critical section
- Lock scope covers entire update-and-redistribute operation
- Use same super_safe_lock pattern as template handler
**Warning signs:** Intermittent share misattribution, jobs sent with wrong coinbase under load, errors only in concurrent testing
**Evidence:** See .planning/research/PITFALLS.md lines 184-230

### Pitfall 4: Future Job Synchronization Gap
**What goes wrong:** Downstream connects between NewTemplate(future=true) and SetNewPrevHash. Channel lacks future job, causing InvalidJobId share rejections when SetNewPrevHash activates it.
**Why it happens:** Job activation is asynchronous. Future template arrives (job 5), downstream connects (receives job 4), SetNewPrevHash arrives (activates job 5), but downstream never received job 5.
**How to avoid:**
- When creating channels, populate BOTH last_active_job AND future_jobs
- Coinbase switching must also send future jobs to all channels
- Test scenario: NewTemplate(future) → API call → OpenChannel → SubmitShares
**Warning signs:** ShareValidationError::InvalidJobId in logs, elevated rejection rate for new connections
**Evidence:** Fixed in commit d9b8ca49 - see .planning/research/PITFALLS.md lines 60-103

## Code Examples

Verified patterns from official sources:

### HTTP Server Startup with State Sharing
```rust
// Source: stratum-apps/src/monitoring/http_server.rs (adapted for API)
use axum::{Router, routing::post, extract::State};
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub async fn start_api_server(
    channel_manager: ChannelManager,
    bind_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/api/coinbase", post(handle_coinbase_update))
        .with_state(channel_manager.clone());

    let listener = TcpListener::bind(bind_addr).await?;
    tracing::info!("API server listening on {}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
```

### Request/Response Types with Validation
```rust
use serde::{Deserialize, Serialize};
use axum::{Json, http::StatusCode, response::IntoResponse};

#[derive(Deserialize)]
pub struct CoinbaseUpdateRequest {
    pub address: String,  // Bitcoin address string
    pub user_id: String,  // User identifier
}

#[derive(Serialize)]
pub struct CoinbaseUpdateResponse {
    pub success: bool,
    pub message: String,
}

// Validation function
fn validate_request(req: &CoinbaseUpdateRequest) -> Result<(), String> {
    // User ID constraints (API-03)
    if req.user_id.len() > 128 {
        return Err("user_id exceeds 128 characters".to_string());
    }
    if !req.user_id.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err("user_id contains invalid characters".to_string());
    }
    Ok(())
}
```

### Address Validation via CoinbaseRewardScript
```rust
// Source: stratum-apps/src/config_helpers/coinbase_output/mod.rs
use stratum_apps::config_helpers::CoinbaseRewardScript;
use miniscript::bitcoin::Network;

pub fn parse_and_validate_address(
    address: &str,
    network: Network,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Wrap in descriptor format (CoinbaseRewardScript expects descriptor syntax)
    let descriptor = format!("addr({})", address);
    let reward_script = CoinbaseRewardScript::from_descriptor(&descriptor)?;

    // Network check (for regtest, all addresses are valid)
    // For production: check ok_for_mainnet()
    if network == Network::Bitcoin && !reward_script.ok_for_mainnet() {
        return Err("Address not valid for mainnet".into());
    }

    // Convert to bytes for storage in coinbase_outputs
    Ok(reward_script.script_pubkey().to_bytes())
}
```

### Complete Handler with Error Handling
```rust
// Source: Derived from axum patterns and pool error handling
use axum::{extract::State, Json, http::StatusCode, response::IntoResponse};

async fn handle_coinbase_update(
    State(channel_manager): State<ChannelManager>,
    Json(req): Json<CoinbaseUpdateRequest>,
) -> impl IntoResponse {
    // Validate user_id constraints (API-03)
    if let Err(e) = validate_request(&req) {
        return (
            StatusCode::BAD_REQUEST,
            Json(CoinbaseUpdateResponse {
                success: false,
                message: e,
            }),
        );
    }

    // Validate Bitcoin address (API-02)
    let coinbase_outputs = match parse_and_validate_address(&req.address, Network::Regtest) {
        Ok(outputs) => outputs,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(CoinbaseUpdateResponse {
                    success: false,
                    message: format!("Invalid Bitcoin address: {}", e),
                }),
            );
        }
    };

    // Update coinbase and broadcast jobs
    match channel_manager.update_coinbase_and_broadcast(coinbase_outputs, req.user_id).await {
        Ok(_) => (
            StatusCode::OK,
            Json(CoinbaseUpdateResponse {
                success: true,
                message: "Coinbase updated successfully".to_string(),
            }),
        ),
        Err(e) => {
            tracing::error!("Failed to update coinbase: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CoinbaseUpdateResponse {
                    success: false,
                    message: "Internal server error".to_string(),
                }),
            )
        }
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual JSON parsing | axum Json extractor | axum 0.6+ | Type-safe deserialization, automatic error handling |
| Extension-based state | State extractor | axum 0.6+ | Compile-time type checking, clearer API |
| std::sync::Mutex | custom_mutex::Mutex | stratum-apps design | Poison error handling, closure-based safety |
| Hardcoded coinbase | Config-based coinbase | Current pool | Dynamic coinbase switching extends this to runtime updates |

**Deprecated/outdated:**
- Extensions for state sharing: Replaced by State extractor (axum 0.6+), more type-safe
- tower 0.4: Replaced by tower 0.5 (breaking changes in Service trait)

## Open Questions

Things that couldn't be fully resolved:

1. **Merkle path validity after coinbase modification**
   - What we know: Template handler modifies coinbase_output[0].value (line 44), suggesting architecture supports coinbase changes
   - What's unclear: Whether channels-sv2 recomputes full merkle root per share or caches intermediate branches
   - Recommendation: MUST test before implementing API - create integration test that switches coinbase, validates share, asserts success. This is a feasibility blocker.

2. **Sub-100ms broadcast latency under concurrent API calls**
   - What we know: Template distribution completes quickly under normal conditions, lock hold times are microseconds
   - What's unclear: Whether concurrent API calls + template updates will cause lock contention exceeding 100ms
   - Recommendation: Implement lock-minimizing pattern (compute outside lock), add tracing spans to measure actual latency, optimize if needed in Phase 3

3. **Standard vs Extended channel job distribution differences**
   - What we know: handle_new_template sends NewMiningJob to standard channels, NewExtendedMiningJob to extended channels (lines 82, 104)
   - What's unclear: Whether coinbase-only updates require different logic than template updates for extended channels
   - Recommendation: Follow template handler pattern exactly - handle both channel types, test with both

## Sources

### Primary (HIGH confidence)
- pool-apps/pool/src/lib/channel_manager/mod.rs - ChannelManager architecture, Arc<Mutex<>> patterns
- pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs - Job recreation pattern (lines 37-134), coinbase modification (lines 43-44)
- stratum-apps/src/custom_mutex.rs - Lock safety patterns, super_safe_lock documentation
- stratum-apps/src/monitoring/http_server.rs - Axum integration example, State pattern (lines 15-92)
- stratum-apps/src/config_helpers/coinbase_output/mod.rs - Bitcoin address validation via CoinbaseRewardScript (lines 22-91)
- https://docs.rs/axum/0.8.8/axum/ - Axum State extractor, JSON handling (verified 2026-02-05)
- https://docs.rs/bitcoin/latest/bitcoin/address/ - Address validation, network checking (verified 2026-02-05)
- .planning/research/PITFALLS.md - Critical pitfalls from codebase analysis (HIGH confidence)
- .planning/research/SUMMARY.md - Architecture patterns, stack recommendations

### Secondary (MEDIUM confidence)
- Sub-100ms latency target: PRD specification, not yet validated in practice. Achievable based on lock hold time estimates (1-50μs) but requires empirical measurement.
- Lock contention under concurrent load: Estimated low based on lock-minimizing pattern, but actual contention depends on API call frequency vs template update rate

### Tertiary (LOW confidence)
- None - all research findings backed by codebase analysis or official documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already integrated (axum, bitcoin, tokio) or standard choices
- Architecture: HIGH - Patterns extracted from working pool code, HTTP integration proven in monitoring server
- Pitfalls: HIGH - All identified from recent bug fix commits (02ea18c7, d9b8ca49) and codebase analysis
- Merkle path validity: MEDIUM - Architecture suggests it works, but MUST test before proceeding

**Research date:** 2026-02-05
**Valid until:** 30 days (stable domain, libraries mature)
