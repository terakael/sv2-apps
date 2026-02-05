# Testing Patterns

**Analysis Date:** 2026-02-05

## Test Framework

**Runner:**
- Rust standard test framework (no external test runner)
- Tests run with `cargo test` command
- Async tests use `#[tokio::test]` from tokio crate

**Assertion Library:**
- Standard Rust assertions: `assert!`, `assert_eq!`, `assert_ne!`
- No external assertion library

**Run Commands:**
```bash
cargo test                   # Run all tests
cargo test --test           # Run integration tests only
cargo test -- --nocapture   # Show output from test prints
cargo test -- --test-threads=1  # Run tests serially
```

**Coverage:**
- codecov.yaml present in root (`.planning/codebase/codecov.yaml`)
- No explicit coverage reporting infrastructure in configs

## Test File Organization

**Location:**
- Co-located with source code using module-level tests
- Tests in same file as implementation code, marked with `#[cfg(test)]` blocks
- Integration tests in separate `integration-tests/` directory

**Naming:**
- Test functions: `test_*` or descriptive names followed by test marker
- Test modules: `#[cfg(test)] mod tests { ... }`
- Integration test files match pattern: `*_integration.rs` or descriptive scenario names

**Structure:**
- Unit tests co-located: `src/file.rs` contains `#[cfg(test)] mod tests { #[test] fn ... }`
- Integration tests: `integration-tests/tests/*.rs` with shared lib support from `integration-tests/lib/mod.rs`
- Test helpers in `integration-tests/lib/` organized by component

## Test Structure

**Suite Organization:**
From `stratum-apps/src/tp_type.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_with_data_dir_mainnet() {
        let result =
            resolve_ipc_socket_path(&BitcoinNetwork::Mainnet, Some(PathBuf::from("/data")));
        assert_eq!(result, Some(PathBuf::from("/data/node.sock")));
    }

    #[test]
    fn missing_data_dir_uses_os_default() {
        let result = resolve_ipc_socket_path(&BitcoinNetwork::Regtest, None);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert!(result.is_some());
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        assert!(result.is_none());
    }
}
```

**Patterns:**
- Setup: Tests initialize state directly; test setup patterns use helper functions for complex scenarios
- Teardown: Implicit through scope (Rust ownership); no explicit cleanup needed for most unit tests
- Assertion: Standard `assert_eq!`, `assert!`, and conditional assertions with `#[cfg(...)]`

## Mocking

**Framework:** No external mocking library; tests use concrete implementations or test doubles

**Patterns:**
From `integration-tests/lib/mock_roles.rs`:
- Create test helper functions that construct test instances
- Build factory functions for test objects: `start_pool()`, `start_jdc()`, `start_jds()`
- Use configuration builders to create test scenarios

Example from `integration-tests/lib/mod.rs`:
```rust
pub fn sv2_tp_config(address: SocketAddr) -> TemplateProviderType {
    TemplateProviderType::Sv2Tp {
        address: address.to_string(),
        public_key: None,
    }
}

pub fn ipc_config(data_dir: std::path::PathBuf, is_signet: bool) -> TemplateProviderType {
    use stratum_apps::tp_type::BitcoinNetwork;
    let network = if is_signet {
        BitcoinNetwork::Signet
    } else {
        BitcoinNetwork::Regtest
    };
    TemplateProviderType::BitcoinCoreIpc {
        network,
        data_dir: Some(data_dir),
        fee_threshold: 0,
        min_interval: 1,
    }
}
```

**What to Mock:**
- External services (use test configurations pointing to test instances)
- Network connections (use `Sniffer` test utility to intercept messages)
- Template providers (use `TemplateProvider` test helper)
- Bitcoin Core (use mock or test instance)

**What NOT to Mock:**
- Core business logic (use real implementations)
- State management (test with actual state mutations)
- Channel operations (use real async-channel)

## Fixtures and Factories

**Test Data:**
From `integration-tests/lib/mod.rs`:
```rust
pub fn start_sniffer(
    identifier: &str,
    upstream: SocketAddr,
    check_on_drop: bool,
    action: Vec<InterceptAction>,
    timeout: Option<u64>,
) -> (Sniffer<'_>, SocketAddr) {
    let listening_address = get_available_address();
    let sniffer = Sniffer::new(
        identifier,
        listening_address,
        upstream,
        check_on_drop,
        action,
        timeout,
    );
    sniffer.start();
    (sniffer, listening_address)
}

pub async fn start_pool(
    template_provider_config: TemplateProviderType,
    supported_extensions: Vec<u16>,
    required_extensions: Vec<u16>,
) -> (PoolSv2, SocketAddr) {
    let listening_address = get_available_address();
    // ... setup configuration ...
    let pool = PoolSv2::new(config);
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        _ = pool_clone.start().await;
    });
    tokio::time::sleep(Duration::from_secs(1)).await;
    (pool, listening_address)
}
```

**Location:**
- Test fixtures: `integration-tests/lib/mod.rs` (main test infrastructure)
- Component-specific fixtures: `integration-tests/lib/{component}/*.rs` (template provider, sniffers, etc.)
- Configuration builders: Helper functions returning configured test objects

**Test Helper Pattern:**
- Factory functions return tuples of (component, address or handle)
- Async setup functions spawn components in background tasks
- Tracing initialized once via `start_tracing()` helper to enable logging in tests

## Coverage

**Requirements:**
- No explicit minimum coverage enforced
- codecov.yaml present but no coverage gates configured

**View Coverage:**
- Standard Rust coverage not directly integrated
- Can generate with `cargo tarpaulin` (external tool)
- Integration tests validate end-to-end behavior across components

## Test Types

**Unit Tests:**
- Located inline in source modules within `#[cfg(test)] mod tests`
- Scope: Individual functions and small components
- Approach: Direct unit testing with minimal setup
- Example: `stratum-apps/src/tp_type.rs` tests for path resolution logic
- Example: `stratum-apps/src/custom_mutex.rs` tests for safe locking behavior
- Example: `stratum-apps/src/config_helpers/coinbase_output/mod.rs` tests for descriptor parsing

**Integration Tests:**
- Located in `integration-tests/tests/*.rs`
- Scope: Multi-component workflows across pool, JDC, template provider, translator, etc.
- Approach:
  - Start components in isolated processes
  - Use message sniffers to verify protocol compliance
  - Assert on message sequences and component state
  - Async/await for test coordination
- Test infrastructure in `integration-tests/lib/` providing:
  - Mock role factories (pool, JDC, JDS, translator, miners)
  - Message interceptors and sniffers
  - Utilities for network address allocation
  - Template provider test helper

**E2E Tests:**
- Not present as separate E2E test suite
- Integration tests serve as E2E validation for critical workflows
- Real component startup (not mocked)

## Common Patterns

**Async Testing:**
From `integration-tests/tests/jd_integration.rs`:
```rust
#[tokio::test]
async fn jds_should_not_panic_if_jdc_shutsdown() {
    start_tracing();
    let (tp, tp_addr) = start_template_provider(None, DifficultyLevel::Low);
    let (_pool, pool_addr) = start_pool(sv2_tp_config(tp_addr), vec![], vec![]).await;
    let (_jds, jds_addr) = start_jds(tp.rpc_info());
    let (sniffer_a, sniffer_addr_a) = start_sniffer("0", jds_addr, false, vec![], None);
    let (jdc, jdc_addr) = start_jdc(
        &[(pool_addr, sniffer_addr_a)],
        sv2_tp_config(tp_addr),
        vec![],
        vec![],
    );
    // Wait for protocol messages
    sniffer_a
        .wait_for_message_type(MessageDirection::ToUpstream, MESSAGE_TYPE_SETUP_CONNECTION)
        .await;
    sniffer_a
        .wait_for_message_type(
            MessageDirection::ToDownstream,
            MESSAGE_TYPE_SETUP_CONNECTION_SUCCESS,
        )
        .await;
    drop(jdc);
    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
    // Verify server still running
    assert!(tokio::net::TcpListener::bind(jdc_addr).await.is_ok());
}
```

**Error Testing:**
From unit tests in `stratum-apps/src/config_helpers/coinbase_output/mod.rs`:
```rust
#[test]
fn fixed_vector_addr() {
    // Valid descriptor succeeds
    assert_eq!(
        CoinbaseRewardScript::from_descriptor(
            "addr(1BvBMSEYstWetqTFn5Au4m4GFg7xJaNVN2)#wdnlkpe8"
        )
        .unwrap()
        .script_pubkey()
        .to_hex_string(),
        "76a91477bff20c60e522dfaa3350c39b030a5d004e839a88ac",
    );
}
```

**Multi-Component Coordination:**
Integration tests coordinate multiple components with:
- `tokio::spawn()` for background component tasks
- Message sniffers to verify inter-component messages
- Async helpers to wait for state changes
- Assertions on protocol message sequences

## Test Data and Scenarios

**Common Test Scenarios:**
- Connection lifecycle tests (connect, exchange messages, disconnect)
- Message protocol compliance (verify correct message types and ordering)
- Multi-component failover (component A crashes, B continues)
- Configuration variations (different template provider types, extensions)

**Test Configuration Constants:**
From `integration-tests/lib/mod.rs`:
```rust
const SHARES_PER_MINUTE: f32 = 120.0;
const POOL_COINBASE_REWARD_DESCRIPTOR: &str = "addr(tb1qa0sm0hxzj0x25rh8gw5xlzwlsfvvyz8u96w3p8)";
const JDS_COINBASE_REWARD_DESCRIPTOR: &str = POOL_COINBASE_REWARD_DESCRIPTOR;
const JDC_COINBASE_REWARD_DESCRIPTOR: &str = "addr(tb1qpusf5256yxv50qt0pm0tue8k952fsu5lzsphft)";
```

---

*Testing analysis: 2026-02-05*
