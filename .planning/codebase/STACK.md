# Technology Stack

**Analysis Date:** 2026-02-05

## Languages

**Primary:**
- Rust 1.85.0 - Core implementation across all applications
  - Edition 2021 for all main packages (pool, translator, jd-client)
  - Edition 2024 for `bitcoin-core-sv2`
  - MSRV: 1.85.0

**Secondary:**
- YAML - CI/CD workflows and Docker configuration
- TOML - Configuration files and Cargo manifests
- Shell - Docker entrypoint scripts and build utilities

## Runtime

**Environment:**
- Tokio 1.44.1 - Async runtime for all network I/O and event handling
- Cap'n Proto (CapNP) 0.21.x - Binary serialization for Bitcoin Core communication

**Package Manager:**
- Cargo - Rust package manager
- Lockfile: `Cargo.lock` present and maintained

## Frameworks

**Core Protocol:**
- stratum-core (from GitHub: `https://github.com/stratum-mining/stratum`, branch `main`)
  - Provides Stratum V2 protocol implementation
  - Optional feature, fetched as git dependency during development
  - Used by: pool, translator, jd-client
- stratum-common (from GitHub rev v1.5.0)
  - Common utilities and network helpers
  - Used by: jd-server, mining-device

**Async/Concurrency:**
- async-channel 1.5.1+ - Multi-producer, multi-consumer channels
- tokio-util 0.7.x - Tokio utilities (codec support)
- futures 0.3.x - Futures abstraction

**Configuration:**
- ext-config (config crate) 0.14.0 - TOML configuration parsing
- clap 4.5.39 - CLI argument parsing with derive macros
- shellexpand 3.1.1 - Environment variable expansion

**Testing:**
- criterion 0.5 - Benchmarking (used in mining-device)
- No standard test framework specified (uses Rust built-in)

## Key Dependencies

**Critical:**
- tokio 1.44.1 - Async runtime, required by all applications
- serde 1.0.89 - Serialization framework with derive support
- secp256k1 0.28.2 - Cryptographic signatures
- tracing 0.1 / tracing-subscriber 0.3 - Structured logging

**Cryptography & Bitcoin:**
- secp256k1 0.28.2 - ECDSA signing
- miniscript 13.0.0 - Bitcoin script descriptors
- bitcoin-capnp-types 0.1.0 - CapNP serialization for Bitcoin types
- sha2 0.10.6 - SHA-256 hashing
- bs58 0.4.0 - Base58 encoding/decoding

**Network:**
- hyper 1.1.0 - HTTP client for Bitcoin Core RPC (pool feature)
- hyper-util 0.1 - Hyper utilities
- http-body-util 0.1 - HTTP body handling
- tokio-util 0.7.10 - Codec support for protocol parsing

**Infrastructure:**
- axum 0.8.7 - Web framework for monitoring endpoints (optional)
- prometheus 0.13 - Prometheus metrics (optional, pool feature)
- utoipa 5.4.0 - OpenAPI documentation (optional)
- utoipa-swagger-ui 9.0.2 - Swagger UI (optional)

**Performance:**
- hotpath 0.9 - CPU cache optimization (pool and translator features)
- dashmap 6.1.0 - Concurrent hash map (translator feature)
- nohash-hasher 0.2.0 - Fast hasher for non-hash-sensitive keys (jd-server)
- hashbrown 0.11 - Optimized hash table (jd-server, with ahash and serde features)

**Utilities:**
- hex 0.4.3 - Hexadecimal encoding/decoding
- base64 0.21.5 - Base64 encoding/decoding (optional)
- serde_json 1.0 - JSON serialization with raw_value support
- rand 0.8.5+ - Random number generation
- once_cell 1.19.0 - Lazy statics (integration tests)
- dirs 6.0 - Platform-specific directory handling

## Configuration

**Environment:**
- Configuration via TOML files located in application directories
- Environment variable substitution in Docker via `envsubst`
- Examples:
  - Pool: `pool-config.toml`
  - JDS: `jds-config.toml`
  - JDC: `jdc-config.toml`
  - Translator: `translator-config.toml`

**Key Environment Variables:**
- `BITCOIN_SOCKET_PATH` - Path to Bitcoin Core IPC socket
- `POOL_*` - Pool configuration
- `JDS_*` - Job Declarator Server configuration
- `JDC_*` - Job Declarator Client configuration
- `TPROXY_*` - Translator proxy configuration
- `RUST_LOG` - Tracing subscriber filter

**Build:**
- Cargo workspaces with per-application manifests
- Feature flags for conditional compilation:
  - `network`, `config`, `core`, `rpc`, `monitoring`
  - `pool`, `jd_client`, `jd_server`, `translator`, `mining_device` - role bundles
  - `sv1` - SV1 protocol support
  - `with_buffer_pool` - Buffer pooling optimization
  - `hotpath` - CPU cache optimization (optional)

## Platform Requirements

**Development:**
- Rust 1.85.0+ (enforced by rust-toolchain.toml)
- Cap'n Proto compiler (libcapnp-dev on Ubuntu, capnp on macOS)
- Linux or macOS (tested on ubuntu-latest, macos-latest)
- WSL2 compatible

**Production:**
- Linux (Docker images built for Linux containers)
- Docker and Docker Compose for orchestration
- Bitcoin Core with IPC support (via Unix socket)
- Network connectivity for p2p mining protocol

**Deployment:**
- Docker containers with multi-stage builds
- Published to Docker Hub: `stratumv2/pool_sv2`, `stratumv2/jd_server`, etc.
- Docker Compose profiles for different deployment scenarios:
  - `pool_apps` - Pool and JDS only
  - `miner_apps` - JDC and Translator only
  - `pool_and_miner_apps` - Full stack
  - `pool_and_miner_apps_no_jd` - Without job declaration

## Workflow and Build Tools

**CI/CD Pipeline:**
- GitHub Actions
- Runs on: ubuntu-latest, macos-latest
- Cargo caching for registry, index, and build artifacts
- Linting: Clippy checks
- Testing: Unit and integration tests

**Code Quality:**
- rustfmt - Code formatting (configured in `rustfmt.toml`)
- Clippy - Linting and best practice checks
- MSRV checking (rust-toolchain pinned at 1.85.0)
- Semver compatibility checking
- Coverage reporting via codecov

**Release Process:**
- Binary releases via GitHub Actions
- Docker image releases with semantic versioning
- Automated tagging and publish workflows

---

*Stack analysis: 2026-02-05*
