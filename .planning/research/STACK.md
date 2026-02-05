# Technology Stack

**Project:** Dynamic Coinbase Switching for SV2 Pool
**Researched:** 2026-02-05
**Confidence:** HIGH

## Recommended Stack

### Core HTTP Framework

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| axum | 0.8.8 | HTTP API server framework | Already in use for monitoring (0.8.7). Designed for tokio, minimal overhead, excellent State extractor pattern for Arc<T> sharing. Macro-free routing, type-safe extractors. |
| tower-http | 0.6.8 | HTTP middleware layer | Provides production middleware (timeout, tracing, error handling). Works seamlessly with axum. Built on tower 0.5.3 which axum uses internally. |
| tokio | 1.49.0 | Async runtime | Already in pool (1.44.1). Can upgrade to 1.49.0 for latest fixes. Pool uses multi-threaded runtime. |

### HTTP Client for Webhooks

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| reqwest | 0.13.1 | Async HTTP client | Built on tokio/hyper. Connection pooling for repeated webhooks. Native timeout support. Retry module available. Industry standard for async HTTP clients. |
| hyper | 1.8.1 | Low-level HTTP (transitive) | Underlying library for both axum and reqwest. Already in stratum-apps (1.1.0 for RPC). Provides connection reuse. |

### Serialization

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| serde | 1.0.89 | Serialization framework | Already in pool. Core trait for JSON serialization. |
| serde_json | 1.0.149 | JSON serialization | Already in stratum-apps (1.0). Fast, zero-copy when possible. Standard for JSON APIs. |

### Synchronization Primitives

| Technology | Version | Purpose | When to Use |
|------------|---------|---------|-------------|
| stratum-apps::custom_mutex::Mutex | - | Existing custom Mutex wrapper | Continue using for channel_manager_data. Already integrated, provides safe_lock/super_safe_lock patterns. |
| tokio::sync::RwLock | 1.49.0 | Async read-write lock | For new async-only state (e.g., webhook queue). Doesn't block executor threads. Multiple concurrent readers. |
| tokio::sync::mpsc | 1.49.0 | Async message passing | For webhook work queue. Non-blocking channel for async tasks. |
| Arc | std | Atomic reference counting | Already used extensively. Continue for shared state across tasks. |

## Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tower | 0.5.3 | Service abstraction | Already transitive via axum. Use for custom middleware if needed (e.g., rate limiting). |
| http | 1.x | HTTP types | Transitive via axum/reqwest. Standard Request/Response types. |
| tracing | 0.1 | Structured logging | Already in pool. Use for API request/response logging, webhook correlation IDs. |
| async-channel | 1.5.1 | Already in pool | Continue using for existing patterns. Bounded channels for backpressure. |

## Installation

Already have most dependencies. Add to pool/Cargo.toml:

```toml
[dependencies]
# Existing - verify versions
tokio = { version = "1.49.0", features = ["full"] }  # upgrade from 1.44.1
serde = { version = "1.0.89", features = ["derive", "alloc"], default-features = false }
serde_json = "1.0.149"
tracing = { version = "0.1" }
async-channel = "1.5.1"

# New - HTTP API server
axum = "0.8.8"  # already in stratum-apps 0.8.7
tower = "0.5.3"
tower-http = { version = "0.6.8", features = ["trace", "timeout", "catch-panic"] }

# New - HTTP client for webhooks
reqwest = { version = "0.13.1", default-features = false, features = [
    "json",           # JSON request/response bodies
    "rustls-tls",     # Use rustls instead of native-tls (no OpenSSL dependency)
    "http2",          # HTTP/2 support
] }
```

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| axum | actix-web 4.12.1 | If you need actor-based concurrency patterns or prefer macro-heavy API. More opinionated, heavier runtime. Not worth switching from existing axum. |
| axum | warp 0.3.x | Warp is older, less active. Axum supersedes it (from same team). |
| axum | rocket 0.5.x | Rocket requires nightly Rust in older versions. Heavier, more "framework" feel. Not compatible with existing tokio patterns. |
| reqwest | raw hyper | Only if you need maximum control over HTTP/2 streams. Reqwest provides better ergonomics for 99% of use cases. |
| stratum custom_mutex | parking_lot 0.12.5 | parking_lot is faster than std::sync but pool already has custom_mutex wrapper. Don't introduce new dependency unless profiling shows lock contention. |
| tokio::sync::RwLock | std::sync::RwLock | NEVER use std RwLock in async code - blocks executor threads. Use tokio primitives. |
| tokio::sync::RwLock | dashmap 7.0.0-rc2 | If HashMap contention becomes bottleneck. DashMap is lock-free concurrent HashMap. Overkill unless proven need. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| std::sync::RwLock / std::sync::Mutex in async handlers | Blocks executor threads, breaks tokio's cooperative scheduling. Causes deadlocks in async contexts. | tokio::sync::RwLock, tokio::sync::Mutex, or stratum custom_mutex if synchronous |
| ureq | Blocking HTTP client, incompatible with tokio async. | reqwest with tokio feature |
| surf | Built for async-std, not tokio. Ecosystem mismatch. | reqwest |
| Thread::spawn for webhook tasks | Doesn't integrate with tokio runtime, loses structured concurrency. | tokio::spawn |
| unwrap() inside custom_mutex locks | Causes mutex poisoning. Custom_mutex docs warn against this. | Return Result, handle outside lock |

## Stack Patterns for Integration

### Pattern 1: Shared State with ChannelManager

**Current pattern (keep this):**

```rust
pub struct ChannelManager {
    pub(crate) channel_manager_data: Arc<Mutex<ChannelManagerData>>,
    // ... other fields
}
```

**For HTTP API handlers:**

```rust
use axum::{
    extract::State,
    Json,
    response::IntoResponse,
};

// App state for axum
struct ApiState {
    channel_manager: ChannelManager,
}

// Handler with State extractor (recommended pattern)
async fn update_coinbase(
    State(state): State<Arc<ApiState>>,
    Json(payload): Json<CoinbaseUpdate>,
) -> impl IntoResponse {
    // Access ChannelManager through Arc<ApiState>
    state.channel_manager.channel_manager_data.super_safe_lock(|data| {
        data.coinbase_outputs = payload.outputs;
    });

    Json(json!({ "status": "updated" }))
}
```

### Pattern 2: Webhook Client Setup

**Create once, reuse across tasks:**

```rust
use reqwest::Client;
use std::time::Duration;

// In pool initialization
let webhook_client = Client::builder()
    .timeout(Duration::from_millis(500))  // <100ms target, 500ms max
    .pool_max_idle_per_host(10)            // Connection pooling
    .http2_prior_knowledge()               // Use HTTP/2 if available
    .build()?;

let webhook_client = Arc::new(webhook_client);
```

**In share handler:**

```rust
// Clone Arc for task, not whole client
let client = webhook_client.clone();
let webhook_url = config.webhook_url.clone();

tokio::spawn(async move {
    let payload = json!({
        "share_id": share_id,
        "timestamp": timestamp,
    });

    match client
        .post(&webhook_url)
        .json(&payload)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            tracing::info!("Webhook delivered successfully");
        }
        Ok(response) => {
            tracing::warn!("Webhook failed: {}", response.status());
        }
        Err(e) => {
            tracing::error!("Webhook error: {}", e);
        }
    }
});
```

### Pattern 3: Integrating HTTP Server with Existing Runtime

**Pool already uses #[tokio::main], reuse it:**

```rust
// In PoolSv2::start() or similar
pub async fn start(self) -> Result<()> {
    // Existing pool startup...

    // Add HTTP API server as another task
    let api_server = start_api_server(
        config.api_listen_address,
        channel_manager.clone(),
    );

    tokio::spawn(async move {
        if let Err(e) = api_server.await {
            tracing::error!("API server error: {}", e);
        }
    });

    // Existing pool tasks continue...
}

async fn start_api_server(
    addr: SocketAddr,
    channel_manager: ChannelManager,
) -> Result<()> {
    let state = Arc::new(ApiState { channel_manager });

    let app = Router::new()
        .route("/api/coinbase", post(update_coinbase))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
```

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| axum 0.8.8 | tokio 1.0+ | Requires tokio runtime. Works with 1.44.1+. |
| reqwest 0.13.1 | tokio 1.0+, hyper 1.x | Built on hyper 1.x. Use rustls-tls feature to avoid OpenSSL. |
| tower-http 0.6.8 | tower 0.5.x, axum 0.8.x | Version alignment is critical. Use 0.6.x with axum 0.8.x. |
| tokio 1.49.0 | All versions 1.x | Upgrading from 1.44.1 to 1.49.0 is safe (semver compatible). |

**Critical compatibility note:** axum 0.8.x requires tower 0.5.x and tower-http 0.6.x. Don't mix with older versions (axum 0.7 used tower 0.4). The stratum-apps already has axum 0.8.7, so we're aligned.

## Lock Strategy Recommendations

Based on existing pool patterns and new requirements:

| Use Case | Recommended | Rationale |
|----------|-------------|-----------|
| ChannelManagerData | stratum custom_mutex::Mutex | Already used. Short critical sections. Wraps std::sync::Mutex with safer API. |
| Coinbase outputs update | Arc<Mutex<CoinbaseData>> | Existing pattern works. Update via super_safe_lock. |
| Webhook queue (new) | tokio::sync::mpsc | Async-native. Bounded channel for backpressure. Doesn't block executor. |
| Read-heavy config (new) | tokio::sync::RwLock | If API needs frequent reads of shared config with rare updates. |
| Per-connection state | No lock needed | Each Downstream already has independent state. |

**Lock-free alternatives consideration:**

- **dashmap:** Only if profiling shows HashMap contention. Current Arc<Mutex<HashMap>> pattern is fine unless >10k downstream connections.
- **crossbeam channels:** Only if mixing sync/async. Stick with async-channel and tokio::sync::mpsc.
- **parking_lot:** Faster than std but pool already has custom_mutex. Don't add dependency unless proven need.

## Critical Path Performance

For <100ms latency requirement:

1. **HTTP API endpoint:** axum with proper timeout middleware (~1-5ms overhead)
2. **Lock acquisition:** custom_mutex with short critical sections (<1ms)
3. **Webhook spawn:** tokio::spawn is non-blocking (~microseconds)
4. **Webhook send:** Fire-and-forget with timeout. Don't block on response.

**Anti-pattern to avoid:** Synchronous HTTP call inside lock. Always spawn task for webhooks.

## Sources

- **crates.io:** Verified versions via `cargo search` (2026-02-05)
  - axum 0.8.8
  - reqwest 0.13.1
  - tokio 1.49.0
  - tower 0.5.3
  - tower-http 0.6.8
  - serde_json 1.0.149
  - parking_lot 0.12.5
  - dashmap 7.0.0-rc2

- **Official documentation:**
  - https://docs.rs/axum/latest/axum/ - State patterns, tokio integration
  - https://docs.rs/reqwest/latest/reqwest/ - Client setup, connection pooling
  - https://docs.rs/tower-http/latest/tower_http/ - Middleware patterns
  - https://docs.rs/tokio/latest/tokio/sync/ - Async synchronization primitives
  - https://docs.rs/parking_lot/latest/parking_lot/ - Performance characteristics

- **Existing codebase:**
  - /home/dan/worktrees/personal/stratum/sv2-apps/main/pool-apps/pool/Cargo.toml
  - /home/dan/worktrees/personal/stratum/sv2-apps/main/stratum-apps/Cargo.toml
  - /home/dan/worktrees/personal/stratum/sv2-apps/main/stratum-apps/src/custom_mutex.rs
  - /home/dan/worktrees/personal/stratum/sv2-apps/main/pool-apps/pool/src/lib/channel_manager/mod.rs

---
*Stack research for: Dynamic Coinbase + Webhooks for SV2 Pool*
*Researched: 2026-02-05*
