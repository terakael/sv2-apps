# Architecture

**Analysis Date:** 2026-02-05

## Pattern Overview

**Overall:** Modular async actor-based architecture with protocol-driven message passing

**Key Characteristics:**
- Tokio-based async runtime with multi-threaded executor (configurable to single-threaded for `hotpath` allocator)
- Message-driven communication between major components via async channels
- Noise-encrypted connections for SV2 protocol communication
- Feature-gated compilation enabling role-specific functionality (pool, JDC, translator, mining-device)
- Centralized task lifecycle management via `TaskManager`
- Standardized error handling with context-aware action mapping (log, disconnect, shutdown, fallback)

## Layers

**Application Layer:**
- Purpose: Entry point and CLI handling, initializes configuration and logging
- Location: `pool-apps/pool/src/main.rs`, `miner-apps/jd-client/src/main.rs`, `miner-apps/translator/src/main.rs`
- Contains: CLI argument processing, config file loading, main event loop orchestration
- Depends on: Config helpers, logging, core application logic
- Used by: End-user binaries

**Business Logic Layer:**
- Purpose: Core role-specific logic (PoolSv2, JobDeclaratorClient, Translator)
- Location: `pool-apps/pool/src/lib/mod.rs`, `miner-apps/jd-client/src/lib/mod.rs`, `miner-apps/translator/src/lib/`
- Contains: Main `struct` definitions (PoolSv2, JobDeclaratorClient, Translator), start methods, task orchestration
- Depends on: Channel managers, downstream/upstream handlers, template receivers, error handling
- Used by: Application layer main functions

**Channel Manager Layer:**
- Purpose: Central state management for mining channels, extranonce allocation, vardiff control
- Location: `pool-apps/pool/src/lib/channel_manager/`, `miner-apps/jd-client/src/lib/channel_manager/`
- Contains: ChannelManager struct, message handlers (mining, template distribution, extensions), channel lifecycle
- Depends on: Downstream connections, upstream connections, template data, SV2 protocol handlers
- Used by: Main app logic, downstream/upstream message handlers

**Connection Layer:**
- Purpose: Manage individual downstream client connections or upstream pool connections
- Location: `pool-apps/pool/src/lib/downstream/`, `miner-apps/jd-client/src/lib/downstream/`, `miner-apps/jd-client/src/lib/upstream/`
- Contains: Downstream/Upstream structs, per-connection state, message frame handling
- Depends on: Noise connection streams, task manager, channel manager
- Used by: Channel manager, I/O task handlers

**Template Reception Layer:**
- Purpose: Fetch and deserialize block templates from external sources (Bitcoin Core or SV2 Template Provider)
- Location: `pool-apps/pool/src/lib/template_receiver/`, `miner-apps/jd-client/src/lib/template_receiver/`
- Contains: Bitcoin Core IPC adapter, SV2 Template Provider client, NewTemplate/SetNewPrevHash forwarding
- Depends on: Bitcoin Core connection (via bitcoin_core_sv2), SV2 protocol handlers
- Used by: Main app loop, channel manager

**Network Helpers Layer:**
- Purpose: Low-level encrypted networking and protocol frame handling
- Location: `stratum-apps/src/network_helpers/`, `stratum-apps/src/utils/`
- Contains: Noise connection management, frame encoding/decoding, SV1 protocol support
- Depends on: stratum-core protocol libraries, tokio networking
- Used by: Connection layer, message handlers

**Configuration Layer:**
- Purpose: Load, parse, and validate configuration from TOML files
- Location: `stratum-apps/src/config_helpers/`
- Contains: TOML parsing, coinbase output parsing, logging initialization
- Depends on: ext-config, serde, tracing
- Used by: Application layer, business logic initialization

**Utilities Layer:**
- Purpose: Shared types, helpers, and cross-cutting concerns
- Location: `stratum-apps/src/` (task_manager, custom_mutex, key_utils, monitoring, rpc)
- Contains: Task lifecycle management, custom synchronization, cryptographic key handling, HTTP metrics server
- Depends on: External crates (tokio, axum, prometheus)
- Used by: All other layers

## Data Flow

**Pool-TemplateProvider Flow:**

1. Application initializes PoolSv2 with config
2. PoolSv2::start() spawns template receiver task (connects to Bitcoin Core or SV2 TP)
3. Template receiver sends NewTemplate/SetNewPrevHash messages via channel to channel manager
4. Channel manager stores latest templates in ChannelManagerData
5. When downstream connects, channel manager sends stored templates to client

**Pool-Downstream Mining Flow:**

1. Downstream client connects to pool via TCP + Noise handshake
2. Pool accepts connection, creates Downstream struct, spawns I/O tasks
3. Downstream sends SetupConnection message → handled by common_message_handler
4. Pool sends mining channels to downstream (standard or extended)
5. Downstream sends Submit message → routed to channel_manager via channel
6. Channel manager validates share, updates vardiff, broadcasts to other downstreams if needed
7. Response routed back via downstream_sender channel to I/O writer task

**JDC-Upstream Mining Flow:**

1. JDC connects to upstream pool via TCP + Noise
2. Upstream module performs SetupConnection handshake
3. JDC receives mining channels from upstream
4. Downstream miners connect to JDC
5. JDC allocates job tokens to downstreams via job_declarator
6. Downstreams declare custom jobs to upstream via JDC
7. Upstream sends NewTemplate to downstream through JDC channel manager

**State Management:**

- **Immutable Configuration**: Loaded once at startup via `Arc<Config>`, shared read-only across all tasks
- **Mutable Channel State**: Protected by `Arc<Mutex<ChannelManagerData>>` for atomic updates during mining operations
- **Connection State**: Per-downstream/upstream in `Arc<Mutex<*Data>>` structs; shared via channels to I/O tasks
- **Broadcast State**: Shutdown signals via `tokio::sync::broadcast` for coordinated graceful termination

## Key Abstractions

**ChannelManager:**
- Purpose: Central orchestrator for all mining channels and share handling
- Examples: `pool-apps/pool/src/lib/channel_manager/mod.rs`, `miner-apps/jd-client/src/lib/channel_manager/mod.rs`
- Pattern: Arc-wrapped Mutex-protected state with per-message handler methods; receives/sends via async channels

**Downstream/Upstream:**
- Purpose: Represent connected peers and their associated mining channels
- Examples: `pool-apps/pool/src/lib/downstream/mod.rs`, `miner-apps/jd-client/src/lib/upstream/mod.rs`
- Pattern: Arc-wrapped connection state paired with channel pair for bidirectional messaging with channel manager

**Message Handlers:**
- Purpose: Process incoming SV2 protocol messages and execute role-specific logic
- Examples: `*_message_handler.rs` modules across pool, jdc, translator
- Pattern: Async methods that parse frames, validate, update state, and generate response messages

**TaskManager:**
- Purpose: Centralized lifecycle management for all spawned tokio tasks
- Location: `stratum-apps/src/task_manager.rs`
- Pattern: Collects JoinHandles, provides join_all() and abort_all() for graceful/forced shutdown

## Entry Points

**Pool Binary:**
- Location: `pool-apps/pool/src/main.rs`
- Triggers: `cargo run --bin pool --manifest-path pool-apps/Cargo.toml`
- Responsibilities: Parse CLI args, load config, initialize logging, create PoolSv2 and call start()

**JDC Binary:**
- Location: `miner-apps/jd-client/src/main.rs`
- Triggers: `cargo run --bin jd-client --manifest-path miner-apps/Cargo.toml`
- Responsibilities: Parse CLI args, load config, initialize logging, create JobDeclaratorClient and call start()

**Translator Binary:**
- Location: `miner-apps/translator/src/main.rs`
- Triggers: `cargo run --bin translator --manifest-path miner-apps/Cargo.toml`
- Responsibilities: Parse CLI args, load config, initialize logging, create Translator and call start()

**Mining Device (library):**
- Location: `miner-apps/mining-device/src/lib/mod.rs`
- Triggers: Used as dependency in integration tests and custom applications
- Responsibilities: Simulates mining hardware for testing, configurable worker threads and nonce generation

**Bitcoin Core Bridge:**
- Location: `bitcoin-core-sv2/src/lib.rs`
- Triggers: Used as dependency in template receiver modules
- Responsibilities: Establish IPC to Bitcoin Core node, forward template updates and solution submissions

## Error Handling

**Strategy:** Context-aware error escalation with semantic action mapping (log, disconnect, shutdown, fallback)

**Patterns:**

- **Generic Error Type with Owner Context**: `PoolError<Owner>` / `JDCError<Owner>` where Owner is phantom type marker (e.g., `ChannelManager`, `Downstream`, `TemplateProvider`)
  - Enables compile-time validation of which error actions are valid for each component
  - Example: `CanDisconnect` trait implemented only on components that can disconnect specific downstream clients

- **Result Alias**: `PoolResult<T, Owner>` = `Result<T, PoolError<Owner>>`
  - Simplifies function signatures and enforces consistent error handling

- **Constructor Methods**:
  - `PoolError::log()` - Log the error, continue operation
  - `PoolError::disconnect()` - Close specific downstream connection
  - `PoolError::shutdown()` - Initiate full application shutdown
  - Example in `pool-apps/pool/src/lib/error.rs` lines 62-94

- **Error Type Conversions**: From stratum-core types (codec, framing, handlers, channels) automatically convert to application-level errors via trait impls

- **Error Propagation in Handlers**: Message handlers return error, calling code examines action field and takes appropriate action (broadcast shutdown, close downstream socket, etc.)

## Cross-Cutting Concerns

**Logging:**
- Framework: `tracing` with `tracing-subscriber` for filtering and output
- Configuration: Loaded from config file `log_dir` setting, initialized at startup via `init_logging()`
- Pattern: Instrumented tasks create spans with location metadata (`#[instrument]` macro, manual span creation)
- Locations: `stratum-apps/src/config_helpers/logging.rs`

**Validation:**
- Input validation: Done at message handler level when parsing incoming frames
- State validation: Channel IDs, downstream IDs checked against HashMaps before access
- Protocol validation: Enforced by stratum-core handler types (e.g., ExtendedChannelError for invalid channel creation)

**Authentication:**
- Noise Protocol: Established during TCP connection handshake, verified by connection layer
- Public key validation: Upstream pool public key verified during JDC connection setup
- Message authenticity: Enforced by Noise protocol encryption, frame signatures validated by codec

**Monitoring:**
- HTTP metrics server (when feature enabled): Exposes channel metrics via Axum + Prometheus
- Location: `stratum-apps/src/monitoring/`
- Provides: Channel statistics, share counts, difficulty levels (SV1 and SV2)
- Pattern: Spawned as separate task, reads from `Arc<Mutex<>>` channel manager data

**Shutdown Coordination:**
- Broadcast channel: `tokio::sync::broadcast` carries `ShutdownMessage` enum
- Capacity: `SHUTDOWN_BROADCAST_CAPACITY` = 4M (handles stacked lags in broadcast queues)
- Pattern: All long-running tasks subscribe to shutdown channel, exit loop when signal received
- Graceful termination: TaskManager.join_all() waits for all tasks before return

