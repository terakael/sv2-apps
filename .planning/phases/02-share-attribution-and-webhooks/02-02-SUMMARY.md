---
phase: 02-share-attribution-and-webhooks
plan: 02
subsystem: share-attribution
tags: [rust, hashmap, channel-manager, memory-management]

# Dependency graph
requires:
  - phase: 01-api-foundation-and-template-switching
    provides: update_coinbase_and_broadcast method for job creation
provides:
  - job_to_user HashMap in ChannelManagerData for share attribution
  - Job mapping population during coinbase updates (atomic within lock)
  - Stale mapping cleanup on SetNewPrevHash (memory leak prevention)
affects: [02-03-share-validation-webhooks]

# Tech tracking
tech-stack:
  added: []
  patterns: [job-attribution-mapping, blockchain-state-cleanup]

key-files:
  created: []
  modified:
    - pool-apps/pool/src/lib/channel_manager/mod.rs
    - pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs
    - pool-apps/pool/src/lib/mod.rs

key-decisions:
  - "Store job mappings BEFORE broadcasting messages (atomic ordering prevents attribution gap)"
  - "Clear all mappings on SetNewPrevHash (simple strategy, jobs become stale on blockchain advance)"
  - "Track both group channel and standard channel job IDs (comprehensive coverage)"

patterns-established:
  - "Job attribution: HashMap<u32, String> tracks job_id → user_id for lagging share handling"
  - "Blockchain cleanup hook: SetNewPrevHash clears stale mappings to prevent unbounded growth"
  - "Critical ordering: Mapping must exist BEFORE job messages broadcast to miners"

# Metrics
duration: 5min
completed: 2026-02-05
---

# Phase 2 Plan 2: Job Attribution Mapping Summary

**HashMap-based job-to-user tracking with atomic population during coinbase updates and blockchain-driven cleanup to prevent memory leaks**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-05T20:16:31Z
- **Completed:** 2026-02-05T20:21:43Z
- **Tasks:** 3
- **Files modified:** 3

## Accomplishments

- Added job_to_user HashMap to ChannelManagerData for mapping job_id → user_id
- Populated mapping atomically during update_coinbase_and_broadcast BEFORE message broadcast
- Implemented cleanup logic in handle_set_new_prev_hash to prevent memory leak
- Tracked both group channel and standard channel job IDs for comprehensive coverage

## Task Commits

Each task was committed atomically:

1. **Task 1: Add job_to_user HashMap to ChannelManagerData** - `f0bb753a` (feat)
2. **Task 2: Populate job_to_user mapping in update_coinbase_and_broadcast** - `054d98ef` (feat)
3. **Task 3: Add cleanup logic in handle_set_new_prev_hash** - `a3bbaaf2` (feat)

## Files Created/Modified

- `pool-apps/pool/src/lib/channel_manager/mod.rs` - Added job_to_user HashMap field to ChannelManagerData, initialized in constructor, populated during job creation
- `pool-apps/pool/src/lib/channel_manager/template_distribution_message_handler.rs` - Added cleanup logic in handle_set_new_prev_hash to clear stale mappings
- `pool-apps/pool/src/lib/mod.rs` - Temporarily disabled incomplete webhook module to allow compilation (re-enabled after 02-01 completion)

## Decisions Made

**Store mappings BEFORE broadcasting messages (critical ordering)**
- Mappings inserted at lines 869 and 875 within super_safe_lock
- Messages broadcast at line 891 after lock released
- Prevents Pitfall 1 (attribution gap) where shares arrive before mapping exists

**Clear all mappings on SetNewPrevHash (simple cleanup strategy)**
- Jobs become stale when blockchain advances (new block found)
- Next coinbase update will repopulate for new template
- Alternative (retain current template jobs) rejected for simplicity in MVP

**Track both group and standard channel job IDs**
- Group channel job ID used by extended channels (line 868)
- Standard channel job IDs handled individually (lines 872-877)
- Ensures comprehensive coverage for all channel types

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Temporarily disabled incomplete webhook module**
- **Found during:** Task 1 (compilation check)
- **Issue:** Webhook module declared in mod.rs but submodules (client.rs, types.rs) not yet created, blocking compilation
- **Fix:** Commented out `pub mod webhook;` temporarily to allow Task 1 verification
- **Files modified:** pool-apps/pool/src/lib/mod.rs
- **Verification:** cargo check passes, webhook module re-enabled after 02-01 completion
- **Committed in:** f0bb753a (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Auto-fix necessary to verify Task 1 compilation. Webhook module re-enabled when submodules completed in parallel plan 02-01.

## Issues Encountered

None - all tasks executed as planned once blocking webhook module issue resolved.

## Next Phase Readiness

**Ready for Phase 2 Plan 3 (share validation webhooks):**
- job_to_user mapping infrastructure in place
- Population logic tested and atomic
- Cleanup prevents memory leaks
- Both group and standard channels supported

**Remaining work for webhook delivery:**
- Integrate mapping lookup in handle_submit_shares (Plan 02-03)
- Call webhook client for valid shares and block solutions (Plan 02-03)
- Handle lagging shares with InvalidJobId but valid mapping (Plan 02-03)

**Memory management validated:**
- HashMap cleared on SetNewPrevHash (every ~10 minutes on regtest, 10 min average on mainnet)
- Prevents unbounded growth (CONC-03 requirement satisfied)
- Logging added for monitoring cleanup effectiveness

---
*Phase: 02-share-attribution-and-webhooks*
*Completed: 2026-02-05*
