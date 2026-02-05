# Codebase Structure

**Analysis Date:** 2026-02-05

## Directory Layout

```
sv2-apps/
├── bitcoin-core-sv2/                    # Bitcoin Core IPC bridge library
│   ├── src/
│   │   ├── lib.rs                       # Main entry point, BitcoinCoreSv2 struct
│   │   ├── bitcoin_core_client.rs       # Cap'n Proto IPC communication
│   │   ├── messages.rs                  # Message type definitions
│   │   └── cancellation_token.rs        # Graceful shutdown support
│   ├── examples/                        # Usage examples for Bitcoin Core integration
│   └── Cargo.toml
│
├── pool-apps/                           # Pool operator applications
│   ├── pool/                            # SV2 Mining Pool implementation
│   │   ├── src/
│   │   │   ├── main.rs                  # Entry point, CLI argument handling
│   │   │   ├── args.rs                  # CLI argument definitions
│   │   │   ├── lib/
│   │   │   │   ├── mod.rs               # PoolSv2 struct and start() method
│   │   │   │   ├── config.rs            # Configuration structures and parsing
│   │   │   │   ├── error.rs             # Error types and context-aware actions
│   │   │   │   ├── status.rs            # Status tracking for channels and monitoring
│   │   │   │   ├── channel_manager/     # Central channel and share management
│   │   │   │   │   ├── mod.rs           # ChannelManager state and lifecycle
│   │   │   │   │   ├── mining_message_handler.rs     # Processes Mining protocol messages
│   │   │   │   │   └── template_distribution_message_handler.rs  # Template handling
│   │   │   │   ├── downstream/          # Downstream client connection handling
│   │   │   │   │   ├── mod.rs           # Downstream struct, I/O task spawning
│   │   │   │   │   ├── common_message_handler.rs     # SetupConnection, protocol negotiation
│   │   │   │   │   └── extensions_message_handler.rs # Extension negotiation
│   │   │   │   ├── template_receiver/   # Block template acquisition
│   │   │   │   │   ├── mod.rs           # Module interface
│   │   │   │   │   ├── bitcoin_core.rs  # Bitcoin Core IPC connection
│   │   │   │   │   └── sv2_tp/
│   │   │   │   │       ├── mod.rs       # SV2 Template Provider client
│   │   │   │   │       └── common_message_handler.rs
│   │   │   │   ├── io_task.rs           # Reader/writer task spawning with shutdown support
│   │   │   │   ├── monitoring.rs        # HTTP metrics exposure
│   │   │   │   └── utils.rs             # Helper functions and shared types
│   │   ├── config-examples/             # Example configurations for testnet/mainnet
│   │   └── Cargo.toml
│   │
│   └── jd-server/                       # Job Declarator Server (excluded from workspace)
│       ├── src/
│       │   ├── main.rs
│       │   ├── args.rs
│       │   └── lib/
│       │       ├── mod.rs
│       │       ├── config.rs
│       │       ├── error.rs
│       │       ├── mempool_builder.rs   # Mempool state synchronization
│       │       ├── job_declaration_handler.rs
│       │       └── template_receiver/
│       └── Cargo.toml
│
├── miner-apps/                          # Miner/client applications
│   ├── jd-client/                       # Job Declarator Client
│   │   ├── src/
│   │   │   ├── main.rs                  # Entry point
│   │   │   ├── args.rs                  # CLI arguments
│   │   │   ├── lib/
│   │   │   │   ├── mod.rs               # JobDeclaratorClient struct and start()
│   │   │   │   ├── config.rs            # Configuration with JD-specific settings
│   │   │   │   ├── error.rs             # JDC error types (supports Fallback action)
│   │   │   │   ├── jd_mode.rs           # JD mode setup (aggregated vs solo)
│   │   │   │   ├── status.rs            # JDC-specific status types
│   │   │   │   ├── channel_manager/     # Central channel state
│   │   │   │   │   ├── mod.rs           # ChannelManager with job declaration support
│   │   │   │   │   ├── downstream_message_handler.rs
│   │   │   │   │   ├── upstream_message_handler.rs  # Receives from pool
│   │   │   │   │   ├── jd_message_handler.rs        # Job declaration protocol
│   │   │   │   │   ├── template_message_handler.rs
│   │   │   │   │   └── extensions_message_handler.rs
│   │   │   │   ├── downstream/          # Local downstream client connections
│   │   │   │   │   ├── mod.rs           # Downstream for JDC
│   │   │   │   │   ├── common_message_handler.rs
│   │   │   │   │   └── extensions_message_handler.rs
│   │   │   │   ├── upstream/            # Upstream pool connection
│   │   │   │   │   ├── mod.rs           # Upstream struct, connection lifecycle
│   │   │   │   │   └── message_handler.rs
│   │   │   │   ├── job_declarator/      # Job declaration logic
│   │   │   │   │   ├── mod.rs           # JobDeclarator state
│   │   │   │   │   └── message_handler.rs
│   │   │   │   ├── template_receiver/   # Template reception from Bitcoin Core or TP
│   │   │   │   │   ├── mod.rs
│   │   │   │   │   ├── bitcoin_core.rs
│   │   │   │   │   └── sv2_tp/
│   │   │   │   │       ├── mod.rs
│   │   │   │   │       └── message_handler.rs
│   │   │   │   ├── io_task.rs
│   │   │   │   ├── monitoring.rs
│   │   │   │   └── utils.rs
│   │   ├── config-examples/
│   │   └── Cargo.toml
│   │
│   ├── translator/                      # SV1↔SV2 Translator Proxy
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── args.rs
│   │   │   ├── lib/
│   │   │   │   ├── mod.rs               # Translator struct
│   │   │   │   ├── config.rs
│   │   │   │   ├── error.rs
│   │   │   │   ├── upstream/            # SV2 pool connection
│   │   │   │   ├── downstream/          # SV1 miner connections
│   │   │   │   │   └── mod.rs
│   │   │   │   ├── sv1/                 # SV1 protocol handling
│   │   │   │   │   ├── sv1_server.rs    # Accepts SV1 miner connections
│   │   │   │   │   ├── difficulty_manager.rs
│   │   │   │   │   └── translator.rs    # SV1↔SV2 message translation
│   │   │   │   └── io_task.rs
│   │   ├── config-examples/
│   │   └── Cargo.toml
│   │
│   ├── mining-device/                   # Mining hardware simulator (excluded from workspace)
│   │   ├── src/
│   │   │   ├── main.rs                  # Binary entry point
│   │   │   └── lib/mod.rs               # Mining simulation logic, fast hasher
│   │   ├── tests/
│   │   │   └── fast_hasher_equivalence.rs
│   │   ├── benches/
│   │   └── Cargo.toml
│   │
│   └── Cargo.toml                       # Workspace root (includes jd-client, translator)
│
├── stratum-apps/                        # Shared application utilities library
│   ├── src/
│   │   ├── lib.rs                       # Module re-exports, feature flags documentation
│   │   ├── config_helpers/              # Configuration parsing and management
│   │   │   ├── mod.rs
│   │   │   ├── toml.rs                  # TOML deserialization helpers
│   │   │   ├── logging.rs               # Tracing subscriber initialization
│   │   │   └── coinbase_output/
│   │   │       ├── mod.rs               # Coinbase output constraint parsing
│   │   │       ├── serde_types.rs       # Serde-friendly types for TOML
│   │   │       └── errors.rs
│   │   ├── network_helpers/             # Network connection utilities
│   │   │   ├── mod.rs                   # Error types
│   │   │   ├── noise_connection.rs      # Noise protocol connection setup
│   │   │   ├── noise_stream.rs          # Encrypted read/write half
│   │   │   └── sv1_connection.rs        # SV1 protocol support (feature gated)
│   │   ├── key_utils/                   # Cryptographic key management
│   │   │   ├── mod.rs                   # Secp256k1 key serialization/deserialization
│   │   │   └── raw_keys.rs              # Raw key handling with no_std support
│   │   ├── utils/                       # Shared types and utilities
│   │   │   ├── mod.rs
│   │   │   ├── types.rs                 # Common type definitions (ChannelId, DownstreamId, etc.)
│   │   │   └── protocol_message_type.rs # Message type classification utilities
│   │   ├── rpc/                         # HTTP RPC client and types (feature gated)
│   │   │   ├── mod.rs
│   │   │   └── mini_rpc_client.rs       # Minimal JSON-RPC 2.0 client for JD Server
│   │   ├── monitoring/                  # HTTP metrics server (feature gated)
│   │   │   ├── mod.rs                   # Module interface
│   │   │   ├── server.rs                # Axum-based HTTP server setup
│   │   │   ├── client.rs                # Client for reading channel metrics
│   │   │   ├── prometheus_metrics.rs    # Prometheus metric definitions
│   │   │   ├── http_server.rs           # Endpoint handlers
│   │   │   └── sv1.rs                   # SV1-specific metrics
│   │   ├── task_manager.rs              # Tokio task lifecycle management
│   │   ├── custom_mutex.rs              # Wrapper around std::sync::Mutex
│   │   ├── tp_type.rs                   # TemplateProviderType enum
│   │   └── coinbase_output_constraints.rs # Message builder helper
│   │
│   └── Cargo.toml                       # Feature definitions for all roles
│
├── integration-tests/                   # End-to-end integration test suite
│   ├── tests/                           # Test binaries
│   │   ├── pool_integration.rs          # Pool ↔ TemplateProvider validation
│   │   ├── jd_integration.rs            # JDC ↔ Pool job declaration flow
│   │   ├── translator_integration.rs    # SV1↔SV2 translation validation
│   │   ├── jd_tproxy_integration.rs     # JDC with tproxy support
│   │   ├── template_provider_integration.rs  # Template distribution
│   │   ├── extensions.rs                # Protocol extension negotiation
│   │   ├── bitcoin_core_ipc_integration.rs   # Bitcoin Core bridge
│   │   ├── jds_block_propagation.rs     # JD Server block propagation
│   │   ├── jdc_block_propagation.rs     # JDC block propagation
│   │   ├── sniffer_integration.rs       # Message interception/inspection
│   │   ├── sv1.rs                       # SV1 protocol features
│   │   └── ... (other test scenarios)
│   │
│   ├── lib/                             # Integration test utilities library
│   │   ├── mod.rs                       # Public test infrastructure
│   │   ├── mock_roles.rs                # Mock downstream/upstream for testing
│   │   ├── interceptor.rs               # Message capture and modification
│   │   ├── template_provider.rs         # Test template provider
│   │   ├── sniffer.rs                   # Protocol message proxy/inspector
│   │   └── ... (other test utilities)
│   │
│   ├── high_diff_chain/                 # Pre-computed blockchain state
│   │   ├── blocks/                      # Serialized block data
│   │   └── chainstate/                  # UTXO set snapshot
│   │
│   ├── .config/                         # Test configuration
│   │   └── nextest.toml                 # Integration test runner config
│   │
│   └── Cargo.toml
│
├── docker/                              # Docker configuration
│   ├── Dockerfile
│   └── config/                          # Docker entrypoint configs
│
├── scripts/                             # Build and deployment scripts
│   └── ... (shell scripts)
│
├── .github/                             # GitHub Actions workflows
│   └── workflows/
│
├── rustfmt.toml                         # Rust code formatting rules
├── rust-toolchain.toml                  # Rust version specification (1.85.0+)
├── codecov.yaml                         # Code coverage configuration
├── README.md                            # Project overview and getting started
├── CONTRIBUTING.md                      # Contribution guidelines
├── PROFILING.md                         # Performance profiling guide
├── RELEASE.md                           # Release procedures
└── LICENSE.md                           # Dual MIT/Apache-2.0 license
```

## Directory Purposes

**bitcoin-core-sv2:**
- Purpose: Bridge between Bitcoin Core and SV2 applications via Cap'n Proto IPC
- Contains: IPC client implementation, message forwarding, template/solution handling
- Key files: `lib.rs` (BitcoinCoreSv2 struct), `bitcoin_core_client.rs` (IPC communication)

**pool-apps/pool:**
- Purpose: Stratum V2 mining pool server implementation
- Contains: Pool business logic, channel management, downstream/template receiver handling
- Key files: `lib/mod.rs` (PoolSv2), `lib/channel_manager/mod.rs` (share routing)

**pool-apps/jd-server:**
- Purpose: Job Declarator Server for coordinated job declaration
- Contains: Job declaration protocol handlers, mempool synchronization
- Key files: `lib/mod.rs`, `lib/job_declaration_handler.rs`

**miner-apps/jd-client:**
- Purpose: Job Declarator Client enabling miners to declare custom templates
- Contains: Upstream pool connection, downstream client management, job declaration logic
- Key files: `lib/mod.rs` (JobDeclaratorClient), `lib/upstream/mod.rs`, `lib/job_declarator/mod.rs`

**miner-apps/translator:**
- Purpose: SV1 protocol translator proxy for bridging old and new mining protocols
- Contains: SV1 server, difficulty management, protocol translation layer
- Key files: `lib/mod.rs` (Translator), `lib/sv1/sv1_server.rs`, `lib/sv1/translator.rs`

**miner-apps/mining-device:**
- Purpose: Mining hardware simulator for development and testing
- Contains: Fast hasher implementation, configurable worker threads, nonce generation
- Key files: `lib/mod.rs` (mining simulation), `main.rs` (binary entry point)

**stratum-apps:**
- Purpose: Reusable library utilities for all SV2 applications
- Contains: Configuration parsing, network utilities, key management, monitoring
- Key files: `lib.rs` (module structure), feature flags

**integration-tests:**
- Purpose: End-to-end validation of all components working together
- Contains: Test scenarios, mock roles, message inspection tools
- Key files: `lib/` (test infrastructure), `tests/` (scenario tests)

## Key File Locations

**Entry Points:**
- `pool-apps/pool/src/main.rs`: Pool binary entry point
- `miner-apps/jd-client/src/main.rs`: JDC binary entry point
- `miner-apps/translator/src/main.rs`: Translator binary entry point
- `miner-apps/mining-device/src/main.rs`: Mining device simulator binary
- `stratum-apps/src/lib.rs`: Utilities library root

**Configuration:**
- `pool-apps/pool/src/lib/config.rs`: Pool configuration structures
- `miner-apps/jd-client/src/lib/config.rs`: JDC configuration structures
- `miner-apps/translator/src/lib/config.rs`: Translator configuration structures
- `stratum-apps/src/config_helpers/mod.rs`: Shared config parsing utilities
- `.planning/codebase/` (when created): Analysis and planning documents

**Core Logic:**
- `pool-apps/pool/src/lib/mod.rs`: PoolSv2 struct and orchestration
- `miner-apps/jd-client/src/lib/mod.rs`: JobDeclaratorClient struct and orchestration
- `miner-apps/translator/src/lib/mod.rs`: Translator struct and orchestration
- `pool-apps/pool/src/lib/channel_manager/mod.rs`: Pool channel/share management
- `miner-apps/jd-client/src/lib/channel_manager/mod.rs`: JDC channel management with job declaration
- `bitcoin-core-sv2/src/lib.rs`: Bitcoin Core bridge interface

**Testing:**
- `integration-tests/tests/pool_integration.rs`: Pool integration test scenarios
- `integration-tests/tests/jd_integration.rs`: JDC integration test scenarios
- `integration-tests/lib/mod.rs`: Test infrastructure and utilities
- `miner-apps/mining-device/tests/fast_hasher_equivalence.rs`: Mining device tests

**Errors and Status:**
- `pool-apps/pool/src/lib/error.rs`: Pool error types and actions
- `miner-apps/jd-client/src/lib/error.rs`: JDC error types and actions
- `pool-apps/pool/src/lib/status.rs`: Pool status tracking
- `miner-apps/jd-client/src/lib/status.rs`: JDC status tracking

## Naming Conventions

**Files:**
- `mod.rs`: Module root file containing primary struct(s) and public API
- `*_message_handler.rs`: Handles specific protocol message type (mining, template, extensions, common)
- `config.rs`: Configuration structures and parsing
- `error.rs`: Error types and conversion logic
- `status.rs`: Status/monitoring types
- `utils.rs`: Helper functions, typically private
- `*_integration.rs`: Integration test scenarios in `integration-tests/tests/`

**Directories:**
- `lib/`: Contains the binary's library crate (private implementation)
- `src/`: Source code root for application
- `config-examples/`: Example configuration files for different networks (mainnet, testnet, signet)
- `tests/`: Test files (integration tests, unit tests)
- `benches/`: Benchmark code
- `.config/`: Tool-specific configuration (e.g., nextest.toml)

**Structs:**
- `PoolSv2`: Main pool application struct
- `JobDeclaratorClient`: Main JDC application struct
- `Translator`: Main translator application struct
- `ChannelManager`: Central channel/share management
- `Downstream`: Represents a downstream client connection
- `Upstream`: Represents an upstream server connection
- `*Data`: State container struct (e.g., `ChannelManagerData`, `DownstreamData`)
- `*Channel`: Communication channel container (e.g., `ChannelManagerChannel`)
- `*Error<Owner>`: Generic error type with phantom owner marker

**Functions:**
- `new()`: Constructor, typically creates struct instance
- `start()`: Main async entry point, orchestrates task spawning and main event loop
- `handle_*_message()`: Message handler for specific protocol message type
- `spawn_*()`: Spawns a new async task (e.g., `spawn_io_tasks()`)
- `process_*()`: CLI argument processing

## Where to Add New Code

**New Feature (e.g., new mining channel type):**
- Primary code: Add handler in appropriate `*_message_handler.rs` file
  - Example: Pool receives new mining message type → add handler to `pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs`
- Tests: Create integration test in `integration-tests/tests/` directory
  - Example: `integration-tests/tests/new_channel_type_integration.rs`
- Configuration: If configurable, update corresponding `config.rs` struct
  - Example: `pool-apps/pool/src/lib/config.rs` if pool-wide configuration needed

**New Component/Module (e.g., new subsystem within an app):**
- Implementation: Create new directory under `lib/`
  - Example: For new pool feature → `pool-apps/pool/src/lib/new_feature/mod.rs`
- Public API: Define in `mod.rs` file
- Integration: Wire into main app struct's start method
  - Example: Update `PoolSv2::start()` in `pool-apps/pool/src/lib/mod.rs` to initialize new component

**Utilities/Helpers (shared across multiple apps):**
- Shared helpers: `stratum-apps/src/utils/mod.rs` or new submodule
  - Example: Add common error conversion → `stratum-apps/src/utils/errors.rs`
- Configuration helpers: `stratum-apps/src/config_helpers/mod.rs`
- Network helpers: `stratum-apps/src/network_helpers/mod.rs`

## Special Directories

**bin/ directories (none currently, using src/main.rs pattern):**
- Each binary-producing crate uses `src/main.rs` and `src/lib/` structure

**Excluded from workspace:**
- `miner-apps/mining-device/`: Not included in workspace (requires special handling)
  - Reason: Imports from stratum-common (older API)
- `pool-apps/jd-server/`: Not included in workspace (under development)
  - Reason: Different feature set, separate cargo workspace

**Generated/Build Artifacts:**
- `target/`: Build outputs (git-ignored, deleted by `cargo clean`)
- `integration-tests/high_diff_chain/blocks/`: Pre-computed blockchain state (committed for test determinism)
- `.planning/codebase/`: Planning documents (generated by `/gsd:map-codebase`)

**Configuration Directories:**
- `config-examples/`: Example TOML files for different networks (mainnet, testnet, signet)
  - Not committed as deployment configs; used as reference for operator setup
- `.config/`: Tool-specific configuration files (nextest.toml for integration test runner)
- `docker/config/`: Docker entrypoint configuration

