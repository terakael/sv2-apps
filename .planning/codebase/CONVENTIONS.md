# Coding Conventions

**Analysis Date:** 2026-02-05

## Naming Patterns

**Files:**
- Module files use `snake_case`: `custom_mutex.rs`, `noise_connection.rs`, `downstream_message_handler.rs`
- Submodules typically named `mod.rs` within directories: `config_helpers/mod.rs`, `network_helpers/mod.rs`
- Test files are inline, suffixed with `#[test]` or `#[tokio::test]` attributes within modules

**Functions:**
- Private functions use `snake_case`: `safe_lock`, `handle_downstream_message`, `run_vardiff`
- Public API functions use `snake_case`: `from_descriptor`, `new`, `start`, `super_safe_lock`
- Async functions use same naming as sync: `handle_pool_message_frame`, `start_template_provider`

**Variables:**
- Local variables and fields use `snake_case`: `downstream_id`, `template_id`, `extranonce_prefix_factory_extended`
- Type-level constants use `UPPER_SNAKE_CASE`: `SHUTDOWN_BROADCAST_CAPACITY`, `JDC_SEARCH_SPACE_BYTES`, `FULL_EXTRANONCE_SIZE`
- Atomic field names use descriptive `snake_case`: `request_id_factory: AtomicU32`, `downstream_id_factory: AtomicUsize`

**Types:**
- Structs use `PascalCase`: `Mutex<T>`, `JDCError<Owner>`, `ChannelManager`, `ExtendedExtranonce`
- Enums use `PascalCase`: `BitcoinNetwork`, `Action`, `TemplateProviderType`
- Type aliases use `PascalCase`: `TemplateId`, `UpstreamJobId`, `DownstreamId`, `VardiffKey`
- Error types use `PascalCase` with `Error` suffix: `JDCErrorKind`, `JDCError<Owner>`
- Marker traits (used for compile-time behavior) use `CanXxx` pattern: `CanDisconnect`, `CanFallback`, `CanShutdown`

## Code Style

**Formatting:**
- Rustfmt is configured via `rustfmt.toml` in repo root
- Key settings:
  - Edition: 2018
  - Imports indent: Block
  - Imports layout: Mixed
  - Imports granularity: Crate
  - Comment width: 100 (default is 80)
  - Wrap comments: enabled
  - Format code in doc comments: enabled
  - Normalize doc attributes: disabled

**Linting:**
- Rust 1.85.0 with clippy enabled (see `rust-toolchain.toml`)
- No clippy configuration file present; uses default clippy rules
- Code uses `#[cfg_attr(not(test), hotpath::measure_all)]` for performance-critical paths

## Import Organization

**Order:**
1. Standard library imports (`use std::...`)
2. External crate imports (ordered alphabetically by crate name)
3. Crate-relative imports (`use crate::...`)
4. Module declarations (`mod ...`, `pub mod ...`)

**Pattern from `miner-apps/jd-client/src/lib/mod.rs`:**
```rust
use std::{net::SocketAddr, sync::Arc, thread::JoinHandle, time::Duration};

use async_channel::{unbounded, Receiver, Sender};
use bitcoin_core_sv2::CancellationToken;
use stratum_apps::{...};
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, error, info, warn};

use crate::{
    channel_manager::ChannelManager,
    config::{...},
    error::JDCErrorKind,
    ...
};

mod channel_manager;
pub mod config;
```

**Path Aliases:**
- No path aliases detected; full module paths are used
- Re-exports used in `lib.rs` files for public API organization (see `stratum-apps/src/lib.rs`)

## Error Handling

**Patterns:**
- **Result Types**: Errors typically wrapped in custom `Result<T, E>` type alias: `pub type JDCResult<T, Owner> = Result<T, JDCError<Owner>>`
- **Custom Error Enums**: Domain-specific error enums with `From` trait implementations for error conversion (see `miner-apps/jd-client/src/lib/error.rs`)
- **Error Actions**: Errors carry metadata about recovery action:
  ```rust
  pub enum Action {
      Log,
      Disconnect(DownstreamId),
      Fallback,
      Shutdown,
  }
  ```
- **Error Construction**: Methods on error types like `JDCError::log()`, `JDCError::disconnect()`, `JDCError::fallback()`, `JDCError::shutdown()` for contextualized error creation
- **Propagation**: Uses `?` operator for error propagation in async functions
- **Mapping**: Errors mapped with `.map_err()` when converting between error types
- **Fallible Operations**: Fallible functions return `Result` explicitly
- **Unwrap Usage**: Limited to setup/initialization code and tests; avoid in production code paths
  - Example: `expect("Invalid coinbase output in config")` only in initialization
  - Used in tests: `.unwrap()`, `.expect("valid ranges")`

## Logging

**Framework:** `tracing` crate with `tracing-subscriber`

**Patterns:**
- Macros used: `debug!`, `info!`, `warn!`, `error!`
- Initialization in tests: `start_tracing()` helper function (see `integration-tests/lib/mod.rs`)
- Setup pattern:
  ```rust
  let env_filter = EnvFilter::try_from_default_env()
      .unwrap_or_else(|_| EnvFilter::new(Level::INFO.to_string()));
  tracing_subscriber::registry()
      .with(env_filter)
      .with(fmt::layer())
      .init();
  ```
- Contextual logging: Log error conditions with `error!(error = ?e, "...")` pattern
- Performance logging: `#[cfg_attr(not(test), hotpath::measure_all)]` for instrumentation

## Comments

**When to Comment:**
- Public items (functions, types, modules) documented with `///` doc comments
- Complex algorithms explained with inline `//` comments
- Design decisions and constraints documented at module or struct level

**JSDoc/TSDoc:**
- Not applicable; uses Rust documentation format instead
- Doc comments use `///` for items and `//!` for modules/crates
- Example from `stratum-apps/src/custom_mutex.rs`:
  ```rust
  /// Custom synchronization primitive for managing shared mutable state.
  ///
  /// This custom mutex implementation builds on [`std::sync::Mutex`]...
  /// ## Advantages
  /// - **Closure-Based Locking:** The `safe_lock` method encapsulates...
  pub struct Mutex<T: ?Sized>(Mutex_<T>);

  impl<T> Mutex<T> {
      /// Mutex safe lock.
      ///
      /// Safely locks the `Mutex` and executes a closer (`thunk`)...
      pub fn safe_lock<F, Ret>(&self, thunk: F) -> Result<Ret, PoisonError<MutexGuard<'_, T>>>
  ```

## Function Design

**Size:**
- Typical functions range from 10-50 lines
- Complex message handlers may exceed 100 lines (see `downstream_message_handler.rs` at 1342 lines)
- Handlers decomposed into smaller methods when possible

**Parameters:**
- Functions prefer explicit parameters over implicit state
- Use structured types instead of multiple primitives: `SocketAddr` vs separate IP and port fields
- Async handlers take `&mut self` for state mutation

**Return Values:**
- Public functions return `Result<T, E>` for fallible operations
- Use custom result types: `JDCResult<T, Owner>` in jd-client
- Async functions return futures: `async fn foo(...) -> Result<T, E>`

## Module Design

**Exports:**
- Public API defined via `pub mod` declarations in parent
- Re-exports common types in `mod.rs` (see `stratum-apps/src/lib.rs` re-exporting monitoring types)
- Crate-level re-exports use `pub use` for convenience

**Barrel Files:**
- `mod.rs` acts as barrel file, re-exporting submodule contents
- Example from `stratum-apps/src/monitoring/mod.rs`:
  ```rust
  pub mod client;
  pub mod http_server;
  pub mod prometheus_metrics;
  pub mod server;
  pub mod sv1;

  pub use client::{
      ClientInfo, ClientMetadata, ClientsMonitoring, ClientsSummary, ...
  };
  pub use http_server::MonitoringServer;
  ```

**Organization Pattern:**
- Feature-gated modules via `#[cfg(feature = "...")]` (see `stratum-apps/src/lib.rs`)
- Private submodules for implementation details: `channel_manager/downstream_message_handler.rs`
- Public facades for API stability: top-level `pub mod` re-exports core functionality

## Async Patterns

**Runtime:** Tokio-based async with `#[tokio::test]` for async tests

**Patterns:**
- Spawn tasks with `tokio::spawn()` for fire-and-forget operations
- Use `select!` for concurrent message handling (see `miner-apps/jd-client/src/lib/channel_manager/mod.rs`)
- Channel usage: `async_channel::Receiver/Sender` for message passing
- Shutdown coordination via `broadcast::Sender<ShutdownMessage>`

**Lifetimes:**
- Extensive use of `'static` lifetime for owned message types: `Message = AnyMessage<'static>`
- Owned references in data structures to avoid lifetime complexity in async contexts

## Sealing and Visibility

**Visibility:**
- Public items marked `pub` explicitly
- Private implementation details kept private
- Marker traits (like `CanDisconnect`) used to control which types can perform certain operations
- Feature-gated visibility using `#[cfg(feature = "...")]` for conditional API exposure

---

*Convention analysis: 2026-02-05*
