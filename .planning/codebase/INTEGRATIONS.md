# External Integrations

**Analysis Date:** 2026-02-05

## APIs & External Services

**Bitcoin Core:**
- Bitcoin Core JSON-RPC API via HTTP
  - SDK/Client: Custom `MiniRpcClient` in `stratum-apps/src/rpc/mini_rpc_client.rs`
  - Auth: Basic authentication (user:password via Authorization header)
  - Methods used:
    - `getrawtransaction` - Fetch transaction data
    - `getrawmempool` - Get pending transaction hashes
    - `submitblock` - Submit mined blocks
    - `getblockchaininfo` - Health checks

**Bitcoin Core IPC (Cap'n Proto):**
- Direct Unix socket communication with Bitcoin Core via Cap'n Proto RPC
- Used in: `bitcoin-core-sv2` crate
- Location: `bitcoin-core-sv2/Cargo.toml` uses `stratum-core` from GitHub
- Purpose: Template Distribution Protocol implementation
- Configuration: `BITCOIN_SOCKET_PATH` environment variable

**Stratum Protocol:**
- Stratum V2 over custom Noise-encrypted TCP connections
- Stratum V1 translation for legacy miner compatibility
- Implementations in: `stratum-apps/src/network_helpers/`
  - `noise_connection.rs` - Noise protocol handshake
  - `noise_stream.rs` - Encrypted stream handling
  - `sv1_connection.rs` - SV1 JSON-RPC protocol

## Data Storage

**Databases:**
- Not used - No SQL, NoSQL, or database integrations detected
- All data is in-memory, ephemeral, or persisted via configuration files

**File Storage:**
- Local TOML configuration files
- Bitcoin Core data directory (accessed via socket/IPC)
- No object storage or external file services

**Caching:**
- In-memory job storage via DashMap (translator: `dashmap 6.1.0`)
- Buffer pools for protocol message handling (`with_buffer_pool` feature)
- Hotpath CPU cache optimization (`hotpath` crate for pool and translator)

## Authentication & Identity

**Auth Provider:**
- Bitcoin Core: Basic HTTP authentication (RPC credentials)
- Stratum V2: Noise protocol with public key infrastructure
- Key Management: `stratum-apps/src/key_utils/mod.rs`
  - Secp256k1 ECDSA key generation and signing
  - Base58 encoding for key serialization

**Authorization:**
- Role-based: Pool, Job Declarator Server, Job Declarator Client, Translator
- Upstream Authority: Public key verification for JDC and Translator
- Environment variables:
  - `JDC_UPSTREAM_AUTHORITY_PUBKEY` - Verify pool authority
  - `TPROXY_UPSTREAM_AUTHORITY_PUBKEY` - Verify upstream pool/JDS

## Monitoring & Observability

**Metrics:**
- Prometheus metrics (optional feature in pool, jd-client, translator)
- HTTP endpoint for metrics scraping on port 9090 (pool), 9091 (jd-client), 9092 (translator)
- Implementation: `stratum-apps/src/monitoring/prometheus_metrics.rs`
- Exported metrics via Axum web framework

**Structured Logging:**
- Tracing framework (`tracing 0.1`, `tracing-subscriber 0.3`)
- Log filtering via `RUST_LOG` environment variable (env-filter)
- Implementations:
  - `stratum-apps/src/config_helpers/logging.rs` - Logging setup
  - Per-app monitoring:
    - `pool-apps/pool/src/lib/monitoring.rs`
    - `miner-apps/jd-client/src/lib/monitoring.rs`
    - `miner-apps/translator/src/lib/monitoring.rs`
    - `miner-apps/translator/src/lib/sv1_monitoring.rs`

**Error Tracking:**
- Not detected - No external error tracking service (Sentry, etc.)
- Errors logged via tracing framework

## CI/CD & Deployment

**Hosting:**
- Docker containers (self-hosted infrastructure)
- Published to Docker Hub: `stratumv2/pool_sv2:main`, `stratumv2/jd_server:main`, etc.

**CI Pipeline:**
- GitHub Actions (primary CI/CD)
- Workflows:
  - `ci.yaml` - Unit and integration tests on every PR/push
  - `integration-tests.yaml` - Full e2e integration testing
  - `coverage.yaml` - Code coverage reporting to codecov.io
  - `docker-release.yaml` - Build and push Docker images
  - `binary-release.yaml` - Release compiled binaries
  - `msrv.yaml` - Minimum supported Rust version check
  - `semver-check.yaml` - Semantic versioning compatibility
  - `release-apps.yaml` - Automated release workflow

**Code Repository:**
- GitHub: `https://github.com/stratum-mining/sv2-apps`
- Primary branch: `main`
- Git workflow: Feature branches → PR review → auto-rebase → main

## Environment Configuration

**Required Environment Variables:**

Bitcoin Configuration:
- `BITCOIN_SOCKET_PATH` - Path to Bitcoin Core IPC socket (e.g., `/root/.bitcoin/node.sock`)

Pool Configuration:
- `POOL_COINBASE_REWARD_SCRIPT` - Recipient address for rewards
- `POOL_SIGNATURE` - Pool identifier string
- `POOL_SHARES_PER_MINUTE` - Target share rate (float)
- `POOL_SHARE_BATCH_SIZE` - Shares per batch (int)
- `POOL_FEE_THRESHOLD` - Fee threshold for filtering (int)
- `POOL_MIN_INTERVAL` - Minimum time between events (int)

Job Declarator Server (JDS) Configuration:
- `JDS_COINBASE_REWARD_SCRIPT` - Recipient address
- `JDS_CORE_RPC_PORT` - Bitcoin Core RPC port (default: 38332)
- `JDS_CORE_RPC_USER` - Bitcoin RPC username
- `JDS_CORE_RPC_PASS` - Bitcoin RPC password

Job Declarator Client (JDC) Configuration:
- `JDC_USER_IDENTITY` - Client identifier
- `JDC_SHARES_PER_MINUTE` - Share rate
- `JDC_SHARE_BATCH_SIZE` - Shares per batch
- `JDC_SIGNATURE` - Signature identifier
- `JDC_COINBASE_REWARD_SCRIPT` - Reward recipient
- `JDC_FEE_THRESHOLD` - Fee threshold
- `JDC_MIN_INTERVAL` - Minimum interval
- `JDC_UPSTREAM_AUTHORITY_PUBKEY` - Pool's public key for verification
- `JDC_POOL_ADDRESS` - Upstream pool IP/hostname
- `JDC_POOL_PORT` - Upstream pool port
- `JDC_UPSTREAM_JDS_ADDRESS` - Job Declarator Server IP/hostname
- `JDC_UPSTREAM_JDS_PORT` - JDS port

Translator Proxy (TPROXY) Configuration:
- `TPROXY_USER_IDENTITY` - Proxy identifier
- `TPROXY_AGGREGATE_CHANNELS` - Enable channel aggregation (boolean)
- `TPROXY_MIN_INDIVIDUAL_MINER_HASHRATE` - Minimum hashrate threshold (float)
- `TPROXY_SHARES_PER_MINUTE` - Share rate
- `TPROXY_ENABLE_VARDIFF` - Variable difficulty support (boolean)
- `TPROXY_UPSTREAM_ADDRESS` - Upstream pool/JDS IP
- `TPROXY_UPSTREAM_PORT` - Upstream port
- `TPROXY_UPSTREAM_AUTHORITY_PUBKEY` - Upstream authority public key

Logging Configuration:
- `RUST_LOG` - Tracing filter (e.g., `debug,stratum_apps=trace`)

**Configuration Files:**
- Pool: `pool-config.toml` (template: `/app/pool-config.toml.template`)
- JDS: `jds-config.toml` (template: `/app/jds-config.toml.template`)
- JDC: `jdc-config.toml` (template: `/app/jdc-config.toml.template`)
- Translator: `translator-config.toml` (template: `/app/proxy-config.toml.template`)

**Secrets Location:**
- Environment variables passed at runtime
- Docker: `docker_env` file (example: `docker/docker_env.example`)
- Git: `.env*` files are gitignored
- No secrets manager integration (HashiCorp Vault, AWS Secrets Manager, etc.)

## Webhooks & Callbacks

**Incoming Webhooks:**
- Not detected - No external webhook endpoints

**Outgoing Webhooks:**
- Not detected - No HTTP callbacks to external services
- Internal communication via Noise-encrypted TCP channels

## Internal Communication Patterns

**Downstream Connections:**
- SV2 mining devices connect via TCP to pool/translator
- Port 34254 - Pool SV2 endpoint
- Port 34264 - JDS SV2 endpoint
- Port 34265 - JDC SV2 endpoint
- Port 34255 - Translator SV2 endpoint for SV1-to-SV2 conversion

**Upstream Connections:**
- JDC connects to JDS for job declaration
- Translator connects to pool or JDS for work retrieval
- All via Noise protocol encryption

**Docker Network:**
- Custom bridge network `sv2_apps` (subnet: 172.28.0.0/16)
- Services communicate via internal network:
  - Pool: 172.28.0.11
  - JDS: 172.28.0.12
  - JDC: 172.28.0.13
  - Translator: 172.28.0.14

---

*Integration audit: 2026-02-05*
