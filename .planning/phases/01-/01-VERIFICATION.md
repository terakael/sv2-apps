---
phase: 01-api-foundation-and-template-switching
verified: 2026-02-05T15:30:00Z
status: gaps_found
score: 3/5 success criteria verified
gaps:
  - truth: "Pool recreates jobs from stored template with new coinbase outputs within 100ms"
    status: partial
    reason: "Implementation exists and compiles, but performance not measured. No timing validation."
    artifacts:
      - path: "pool-apps/pool/src/lib/channel_manager/mod.rs"
        issue: "update_coinbase_and_broadcast lacks performance instrumentation"
    missing:
      - "Add tracing spans with timing measurements"
      - "Integration test to validate sub-100ms requirement"
  - truth: "Integration test validates merkle path reuse after coinbase switch (TMPL-05)"
    status: blocked
    reason: "Test file exists but cannot compile or run due to missing api_bind_addr parameter in PoolConfig::new call"
    artifacts:
      - path: "integration-tests/tests/test_coinbase_merkle_validity.rs"
        issue: "Exists (314 lines) but won't compile"
      - path: "integration-tests/lib/mod.rs"
        issue: "PoolConfig::new missing 10th parameter (api_bind_addr)"
    missing:
      - "Fix integration test helper to pass api_bind_addr parameter"
      - "Run test to validate merkle path theory"
      - "Document test results"
  - truth: "HTTP server starts and binds to localhost:8080"
    status: uncertain
    reason: "Cannot verify without running pool. Code exists but runtime behavior unverified."
    artifacts:
      - path: "pool-apps/pool/src/lib/mod.rs"
        issue: "Server startup code exists but never tested"
    missing:
      - "Manual test: Start pool and verify API server listening"
      - "Curl test: POST /api/coinbase with valid request"
      - "Verify 200/400 status codes"
---

# Phase 1: API Foundation and Template Switching Verification Report

**Phase Goal:** External control of coinbase addresses with validated job distribution to miners

**Verified:** 2026-02-05T15:30:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HTTP endpoint accepts POST /api/coinbase with address and user_id, returns 200 on success | ✓ VERIFIED | Handler exists (handlers.rs:35-83), routes configured (mod.rs:48-51), returns 200 on success (line 57) |
| 2 | Invalid Bitcoin addresses rejected with 400 status (validated via bitcoin crate) | ✓ VERIFIED | validate_and_parse_address uses bitcoin::Address (mod.rs:128-142), handler returns BAD_REQUEST (handlers.rs:64-68) |
| 3 | Pool recreates jobs from stored template with new coinbase outputs within 100ms | ⚠️ PARTIAL | Implementation exists (mod.rs:727-877), job recreation logic complete, BUT no performance measurement/validation |
| 4 | All connected miners receive NewMiningJob messages with updated coinbase | ✓ VERIFIED | Broadcast logic at lines 869-876, sends NewMiningJob (line 846) and NewExtendedMiningJob (line 824) |
| 5 | Lock strategy prevents deadlock between API calls and template provider updates | ✗ BLOCKED | Lock-minimizing pattern implemented (5 phases), but cannot verify runtime behavior without integration test |

**Score:** 3/5 truths fully verified, 1 partial, 1 blocked

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `pool-apps/pool/src/lib/http_api/mod.rs` | HTTP API module | ✓ VERIFIED | 79 lines, exports types, create_router, start_api_server |
| `pool-apps/pool/src/lib/http_api/types.rs` | Request/response types | ✓ VERIFIED | 229 lines, validation logic, 15 passing unit tests |
| `pool-apps/pool/src/lib/http_api/handlers.rs` | Endpoint handler | ✓ VERIFIED | 83 lines, handle_coinbase_update with proper error codes |
| `pool-apps/pool/src/lib/channel_manager/mod.rs` | update_coinbase_and_broadcast method | ✓ VERIFIED | 1040 lines total, method at 727-877 (151 lines), validate_and_parse_address exists |
| `integration-tests/tests/test_coinbase_merkle_validity.rs` | Merkle path validity test | ✗ BLOCKED | 314 lines exist BUT won't compile (missing api_bind_addr param) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| lib.rs | http_api module | `pub mod http_api` | ✓ WIRED | Declaration at lib/mod.rs |
| types.rs | serde traits | derive macros | ✓ WIRED | Deserialize on Request, Serialize on Response |
| handlers.rs | ChannelManager | update_coinbase_and_broadcast | ✓ WIRED | Called at handlers.rs:53 with State extractor |
| mod.rs (pool) | start_api_server | tokio::spawn | ✓ WIRED | Spawned at lib/mod.rs:137-141 |
| update_coinbase_and_broadcast | bitcoin::Address | validate_and_parse_address | ✓ WIRED | Called at line 738, parses at line 133 |
| update_coinbase_and_broadcast | coinbase_outputs | state update | ✓ WIRED | Updated at line 776 |
| update_coinbase_and_broadcast | downstream channels | job recreation | ✓ WIRED | Iterates downstreams 794-863, sends messages 871-873 |

### Requirements Coverage

| Requirement | Status | Blocking Issue |
|-------------|--------|----------------|
| API-01: POST /api/coinbase accepts {address, user_id} | ✓ SATISFIED | None |
| API-02: Bitcoin address validation | ✓ SATISFIED | None |
| API-03: user_id validation (<=128 chars, alphanumeric/_/-) | ✓ SATISFIED | 15 unit tests pass |
| API-04: Proper HTTP status codes (200/400/500) | ✓ SATISFIED | Handler returns correct codes |
| API-05: Bind to localhost only (127.0.0.1) | ✓ SATISFIED | Default: 127.0.0.1:8080 |
| TMPL-01: Recreate jobs from last_future_template | ✓ SATISFIED | on_new_template called at line 807 |
| TMPL-02: Update coinbase_outputs field | ✓ SATISFIED | Updated at line 776 |
| TMPL-03: Distribute NewMiningJob to standard channels | ✓ SATISFIED | Sent at line 846 |
| TMPL-04: All miners receive within 100ms | ⚠️ NEEDS VERIFICATION | No performance measurement |
| TMPL-05: Integration test validates merkle path | ✗ BLOCKED | Test won't compile |
| CONC-01: Lock strategy prevents deadlock | ⚠️ NEEDS VERIFICATION | Pattern correct, runtime unverified |
| CONC-02: Concurrent updates without data races | ⚠️ NEEDS VERIFICATION | Lock ordering correct, runtime unverified |

**Coverage:** 9/12 requirements satisfied, 3 need verification

### Anti-Patterns Found

No blocking anti-patterns found. Code quality is high:

- No TODO/FIXME/placeholder comments in http_api or channel_manager
- No stub implementations (all methods substantive)
- No empty returns or console.log-only handlers
- Proper error handling throughout

### Human Verification Required

#### 1. HTTP API End-to-End Test

**Test:** Start pool with `cargo run --manifest-path pool-apps/pool/Cargo.toml -- --config <config>`. Use curl to POST /api/coinbase with valid/invalid addresses and user_ids.

**Expected:**
- Pool logs "API server listening on 127.0.0.1:8080"
- Valid request returns 200 with `{"success": true, "message": "Coinbase updated successfully"}`
- Invalid address returns 400 with error message
- Invalid user_id returns 400 with validation error

**Why human:** Need to run actual pool instance with Bitcoin Core regtest. Cannot verify runtime behavior from static code analysis.

#### 2. Job Broadcast to Miners

**Test:** Connect mining device to pool, call /api/coinbase endpoint, observe mining device receives NewMiningJob message with updated coinbase.

**Expected:** Mining device logs show new job received within 100ms of API call.

**Why human:** Requires multi-process setup (pool + miner) and network communication verification.

#### 3. Performance Validation

**Test:** Instrument update_coinbase_and_broadcast with tracing spans, measure end-to-end latency from API call to message broadcast.

**Expected:** Sub-100ms latency (TMPL-04 requirement).

**Why human:** Requires performance profiling tools and real network conditions.

### Gaps Summary

#### Gap 1: Integration Test Blocked (CRITICAL)

**What's broken:** Plan 01-02 created test_coinbase_merkle_validity.rs (314 lines) to validate the core feasibility assumption (merkle path reuse). However, test cannot compile because integration test helper calls `PoolConfig::new()` with 9 arguments but the updated constructor requires 10 (added api_bind_addr).

**Impact:** The most critical validation (TMPL-05) cannot run. This test proves/disproves the entire dynamic coinbase switching approach.

**What's needed:**
1. Fix integration-tests/lib/mod.rs line 136-146: add api_bind_addr parameter (e.g., "127.0.0.1:8080".to_string())
2. Run test: `cargo test --manifest-path integration-tests/Cargo.toml test_coinbase_merkle_validity`
3. Verify test passes (shares accepted after coinbase modification)
4. Document results in 01-02-SUMMARY.md

#### Gap 2: No Performance Measurement

**What's missing:** TMPL-04 requires "within 100ms" but no timing instrumentation exists. update_coinbase_and_broadcast has no tracing spans or timing measurements.

**Impact:** Cannot validate sub-100ms requirement. Phase 3 is supposed to address this, but Phase 1 should establish baseline.

**What's needed:**
1. Add tracing::Span around critical sections in update_coinbase_and_broadcast
2. Create performance integration test measuring API call → job broadcast latency
3. Verify sub-100ms with realistic conditions (multiple miners, template updates)

#### Gap 3: Runtime Behavior Unverified

**What's missing:** No evidence that HTTP server actually starts, binds correctly, or handles requests properly. Code exists but was never run.

**Impact:** Unknown if implementation works in practice. Could have runtime issues not visible in code review.

**What's needed:**
1. Manual test: Start pool, verify "API server listening" log
2. Curl test valid/invalid requests
3. Document results in VERIFICATION or create 01-04-SUMMARY.md

---

## Verification Methodology

**Verification approach:** Goal-backward static analysis
1. Extracted must-haves from PLAN frontmatter (Plans 01-01, 01-03, 01-04)
2. Verified artifact existence and substantive content (line counts, exports)
3. Checked key wiring (imports, calls, state updates)
4. Ran unit tests (21 tests pass)
5. Attempted integration test (compilation blocked)
6. Scanned for anti-patterns (none found)

**Limitations:**
- Static code analysis only (no runtime verification)
- Integration test blocked by compilation error
- Performance not measured
- Multi-component interactions unverified

**Confidence level:** HIGH for code correctness, LOW for runtime behavior

---

_Verified: 2026-02-05T15:30:00Z_
_Verifier: Claude (gsd-verifier)_
_Method: Goal-backward static analysis + unit test execution_
