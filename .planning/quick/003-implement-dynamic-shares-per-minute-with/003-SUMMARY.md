---
phase: quick-003
plan: 01
subsystem: mining-difficulty
type: execute
completed: 2026-02-06
duration: 6min
status: complete

tags:
  - stratum-v2
  - vardiff
  - dynamic-difficulty
  - api
  - backend

dependencies:
  requires:
    - quick-002
  provides:
    - dynamic-shares-per-minute-api
    - immediate-target-recalculation
  affects:
    - none

tech-stack:
  added: []
  patterns:
    - centralized-shares-per-minute-state
    - immediate-vardiff-recalculation
    - fire-and-forward-messaging

key-files:
  created: []
  modified:
    - pool-apps/pool/src/lib/channel_manager/mod.rs
    - pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs
    - pool-apps/pool/src/lib/http_api/types.rs
    - pool-apps/pool/src/lib/http_api/handlers.rs
    - pool-apps/pool/src/lib/http_api/mod.rs

decisions:
  - id: QUICK-003-01
    what: Move shares_per_minute to ChannelManagerData instead of per-channel updates
    why: Channels from stratum library don't support runtime updates; vardiff reads from channels but we can override the value in calculations
    when: 2026-02-06
    alternatives:
      - rejected: Modify channel structs to support set_shares_per_minute
        reason: Would require forking stratum library
      - rejected: Wait for vardiff loop to pick up changes
        reason: Requires immediate recalculation for responsive difficulty adjustments
  - id: QUICK-003-02
    what: Use f32 for shares_per_minute in API instead of f64
    why: SharesPerMinute type alias is f32 in stratum library
    when: 2026-02-06
    alternatives:
      - rejected: Convert f64 to f32 in handler
        reason: Unnecessary complexity, better to match library types directly
---

# Quick Task 003: Dynamic Shares Per Minute Implementation

**One-liner:** Dynamic shares_per_minute API with immediate vardiff recalculation for responsive difficulty adjustments based on active user count

## Objective

Implement dynamic shares_per_minute updates with dedicated API endpoint - add current_shares_per_minute to ChannelManagerData, create /update-shares-per-minute endpoint, and implement immediate target recalculation when updated.

Purpose: Enable backend to dynamically adjust mining difficulty based on active user count, maintaining consistent share submission rate as workload changes.

## What Was Built

### 1. Centralized Shares Per Minute State
- Moved shares_per_minute from ChannelManager struct to ChannelManagerData as current_shares_per_minute
- Initialized from config.shares_per_minute() on startup
- Future channel creation automatically uses current_shares_per_minute value
- Removed shares_per_minute from ChannelManager Debug impl

### 2. Update Method with Immediate Recalculation
- Implemented ChannelManager::update_shares_per_minute(new_shares_per_minute) method
- Updates current_shares_per_minute in ChannelManagerData atomically
- Iterates all downstream channels (standard and extended)
- For each channel:
  - Gets current hashrate and target
  - Calls vardiff.try_vardiff() with NEW shares_per_minute value
  - Updates channel with new hashrate (which recalculates target)
  - Sends SetTarget message immediately to miner
- Returns count of updated channels

### 3. HTTP API Endpoint
- Created SharesPerMinuteUpdateRequest with validation:
  - shares_per_minute must be positive
  - shares_per_minute must be >= 0.1 (minimum reasonable)
  - shares_per_minute must be <= 60.0 (maximum reasonable)
  - Uses f32 to match SharesPerMinute type alias
- Created SharesPerMinuteUpdateResponse with success/error constructors
- Implemented handle_shares_per_minute_update handler:
  - Validates request
  - Calls ChannelManager.update_shares_per_minute()
  - Returns 200/400/500 status codes appropriately
- Registered POST /update-shares-per-minute route

## Implementation Pattern

### Vardiff Calculation Override
Key insight: Channels store shares_per_minute internally, but we can override it during vardiff calculation:

```rust
// Instead of using channel.get_shares_per_minute(), pass NEW value directly
let hashrate = channel.get_nominal_hashrate();
let current_target = channel.get_target();

// Use NEW shares_per_minute for immediate recalculation
match vardiff_state.try_vardiff(hashrate, current_target, new_shares_per_minute) {
    Ok(Some(new_hashrate)) => {
        channel.update_channel(new_hashrate, None)?;
        send_set_target_message(channel.get_target());
    }
}
```

This approach:
- Doesn't require modifying stratum library channel structs
- Immediately recalculates targets based on new shares_per_minute
- Future vardiff cycles continue using the new value (stored in ChannelManagerData)

### Message Forwarding Pattern
Followed existing pattern from update_coinbase_and_broadcast:

```rust
let messages = self.channel_manager_data.super_safe_lock(|data| {
    let mut messages = Vec::new();
    // ... build messages inside lock ...
    Ok(messages)
})?;

// Send messages outside lock using .forward()
for message in messages {
    message.forward(&self.channel_manager_channel).await;
}
```

## Testing Strategy

Manual testing approach:
1. Start pool with default shares_per_minute (e.g., 6.0)
2. Connect miners and observe initial targets
3. Call POST /update-shares-per-minute with {"shares_per_minute": 2.0}
4. Observe SetTarget messages sent immediately to all channels
5. Verify new shares arrive at expected rate (lower difficulty)
6. Verify validation: try negative, zero, > 60.0 values

## Deviations from Plan

None - plan executed exactly as written.

## Next Phase Readiness

### Blockers
None.

### Concerns
None - implementation is straightforward and follows existing patterns.

### Integration Points
- Backend time-sharing service will call /update-shares-per-minute when user count changes
- Formula: shares_per_minute = base_rate / active_users (e.g., 6.0 / 3 = 2.0)
- API is synchronous - returns after targets are updated and messages sent

## Commits

| Commit | Type | Description |
|--------|------|-------------|
| e2eb50ac | refactor | Move shares_per_minute to ChannelManagerData |
| eccb1a83 | feat | Implement update_shares_per_minute method |
| 1886735d | feat | Add /update-shares-per-minute API endpoint |

## Files Modified

- pool-apps/pool/src/lib/channel_manager/mod.rs (175 lines added)
- pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs (2 lines changed)
- pool-apps/pool/src/lib/http_api/types.rs (67 lines added)
- pool-apps/pool/src/lib/http_api/handlers.rs (59 lines added)
- pool-apps/pool/src/lib/http_api/mod.rs (4 lines changed)

## Success Criteria

- [x] current_shares_per_minute field exists in ChannelManagerData
- [x] shares_per_minute removed from ChannelManager struct
- [x] current_shares_per_minute initialized from config.shares_per_minute()
- [x] StandardChannel and ExtendedChannel creation read from data.current_shares_per_minute
- [x] update_shares_per_minute method exists and is public
- [x] update_shares_per_minute updates all existing channel targets immediately
- [x] update_shares_per_minute sends SetTarget messages to all channels
- [x] /update-shares-per-minute endpoint exists and validates input
- [x] Validation enforces positive value and reasonable range (0.1-60.0)
- [x] API endpoint calls update_shares_per_minute method
- [x] cargo check --package pool passes
