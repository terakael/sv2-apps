---
phase: quick-002
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - pool-apps/pool/src/lib/channel_manager/mod.rs
  - pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs
  - pool-apps/pool/src/lib/api_server.rs
autonomous: true

must_haves:
  truths:
    - "pool_tag field accepted in /api/coinbase request"
    - "pool_tag validates max 100 chars and printable ASCII"
    - "pool_tag initialized from config pool_signature at startup"
    - "channels created with pool_tag from ChannelManagerData"
  artifacts:
    - path: "pool-apps/pool/src/lib/channel_manager/mod.rs"
      provides: "ChannelManagerData with current_pool_tag field"
      contains: "current_pool_tag: String"
    - path: "pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs"
      provides: "Channel creation using ChannelManagerData pool_tag"
      pattern: "data\\.current_pool_tag"
    - path: "pool-apps/pool/src/lib/api_server.rs"
      provides: "pool_tag validation in /api/coinbase endpoint"
      pattern: "validate.*pool_tag"
  key_links:
    - from: "pool-apps/pool/src/lib/api_server.rs"
      to: "ChannelManagerData.current_pool_tag"
      via: "update_coinbase_and_broadcast stores optional pool_tag"
      pattern: "current_pool_tag\\s*="
    - from: "pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs"
      to: "ChannelManagerData.current_pool_tag"
      via: "StandardChannel and ExtendedChannel read pool_tag from data"
      pattern: "data\\.current_pool_tag\\.clone\\(\\)"
---

<objective>
Implement pool_tag feature for dynamic coinbase API - move pool_tag from ChannelManager struct to ChannelManagerData, add pool_tag field to /api/coinbase endpoint with validation, initialize from config pool_signature at startup, and update channel creation to use dynamic pool_tag.

Purpose: Enable per-user customization of pool identification string that appears in coinbase scriptSig (visible on block explorers).

Output: Working pool_tag feature with validation, API integration, and channel creation updates.
</objective>

<execution_context>
@/home/dan/.claude/get-shit-done/workflows/execute-plan.md
@/home/dan/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@/home/dan/worktrees/personal/stratum/sv2-apps/.docs/PRD-DYNAMIC-COINBASE.md

**From PRD (lines 129-134):**
- pool_tag is optional field in /api/coinbase request
- Max length: 100 characters (scriptSig space constraint)
- UTF-8 string, any printable characters (specifically ASCII graphic + space based on validation code at line 588)
- If omitted, keeps current pool_tag value
- Default value loaded from config pool_signature at startup

**Current state:**
- pool_tag_string exists in ChannelManager struct (line 108 in mod.rs)
- Initialized from config.pool_signature() (line 218 in mod.rs)
- Passed to StandardChannel::new_for_pool (line 159 in mining_message_handler.rs)
- Passed to ExtendedChannel::new_for_pool (line 326 in mining_message_handler.rs)

**Required changes:**
1. Move pool_tag from ChannelManager to ChannelManagerData as current_pool_tag
2. Initialize current_pool_tag from config.pool_signature() in ChannelManagerData::new()
3. Update channel creation to read from data.current_pool_tag instead of self.pool_tag_string
4. Add pool_tag validation in api_server.rs (max 100 chars, ASCII graphic + space)
5. Update current_pool_tag in update_coinbase_and_broadcast when pool_tag provided
</context>

<tasks>

<task type="auto">
  <name>Move pool_tag to ChannelManagerData and initialize from config</name>
  <files>pool-apps/pool/src/lib/channel_manager/mod.rs</files>
  <action>
    1. Add current_pool_tag: String field to ChannelManagerData struct (after current_user_id field around line 90)
    2. Remove pool_tag_string: String field from ChannelManager struct (line 108)
    3. In ChannelManagerData initialization (find where coinbase_outputs is set), initialize current_pool_tag from config.pool_signature():
       - Look for where ChannelManagerData is constructed
       - Add: current_pool_tag: config.pool_signature().to_string()
    4. Remove pool_tag_string initialization from ChannelManager::new() (line 218)
    5. Remove pool_tag_string from Debug impl for ChannelManager (line 986)

    Note: Do NOT modify the config.rs file or add pool_tag field to config - pool_signature already exists and provides the default value.
  </action>
  <verify>
    cargo check --package pool
    grep -n "current_pool_tag" pool-apps/pool/src/lib/channel_manager/mod.rs
    grep -c "pool_tag_string" pool-apps/pool/src/lib/channel_manager/mod.rs
  </verify>
  <done>
    - current_pool_tag field exists in ChannelManagerData
    - pool_tag_string removed from ChannelManager
    - current_pool_tag initialized from config.pool_signature()
    - cargo check passes
  </done>
</task>

<task type="auto">
  <name>Update channel creation to use ChannelManagerData pool_tag</name>
  <files>pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs</files>
  <action>
    Update all channel creation calls to read pool_tag from ChannelManagerData instead of ChannelManager:

    1. Find StandardChannel::new_for_pool call (around line 159)
       - Change: self.pool_tag_string.clone()
       - To: data.current_pool_tag.clone()

    2. Find ExtendedChannel::new_for_pool call (around line 326)
       - Change: self.pool_tag_string.clone()
       - To: data.current_pool_tag.clone()

    Note: The 'data' parameter should already be available in the closure context where channels are created. If not available, it's passed as mutable reference to the closure.
  </action>
  <verify>
    cargo check --package pool
    grep -n "data.current_pool_tag" pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs
    grep -c "self.pool_tag_string" pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs
  </verify>
  <done>
    - StandardChannel creation uses data.current_pool_tag
    - ExtendedChannel creation uses data.current_pool_tag
    - No references to self.pool_tag_string remain
    - cargo check passes
  </done>
</task>

<task type="auto">
  <name>Add pool_tag validation to API endpoint</name>
  <files>pool-apps/pool/src/lib/api_server.rs</files>
  <action>
    Locate the update_coinbase_and_broadcast method in api_server.rs and add pool_tag validation:

    1. Find where address and user_id are extracted from request payload
    2. Extract optional pool_tag field: let pool_tag = payload.get("pool_tag").and_then(|v| v.as_str());
    3. Add validation BEFORE calling update_coinbase_and_broadcast:
       ```rust
       if let Some(tag) = pool_tag {
           // Max 100 characters per PRD line 131
           if tag.len() > 100 {
               return Err(PoolError::new(
                   PoolErrorKind::InvalidPoolTag,
                   "Pool tag exceeds 100 characters"
               ));
           }
           // ASCII graphic + space per PRD line 588
           if !tag.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
               return Err(PoolError::new(
                   PoolErrorKind::InvalidPoolTag,
                   "Pool tag contains invalid characters (only ASCII graphic + space allowed)"
               ));
           }
       }
       ```
    4. Pass pool_tag to update_coinbase_and_broadcast call (may require signature update)
    5. In update_coinbase_and_broadcast implementation (in mod.rs), add logic to update data.current_pool_tag if pool_tag.is_some():
       ```rust
       if let Some(tag) = pool_tag {
           data.current_pool_tag = tag.to_string();
       }
       ```

    Note: May need to add InvalidPoolTag variant to PoolErrorKind enum if not already present.
  </action>
  <verify>
    cargo check --package pool
    grep -n "pool_tag" pool-apps/pool/src/lib/api_server.rs
    grep -n "current_pool_tag =" pool-apps/pool/src/lib/channel_manager/mod.rs
  </verify>
  <done>
    - pool_tag field extracted from API request
    - Validation enforces max 100 chars
    - Validation enforces ASCII graphic + space only
    - pool_tag passed to update_coinbase_and_broadcast
    - current_pool_tag updated in ChannelManagerData when provided
    - cargo check passes
  </done>
</task>

</tasks>

<verification>
Run these checks to verify complete implementation:

1. Compile check:
   ```
   cargo check --package pool
   ```

2. Verify struct changes:
   ```
   grep -A 5 "pub struct ChannelManagerData" pool-apps/pool/src/lib/channel_manager/mod.rs | grep current_pool_tag
   grep "pool_tag_string" pool-apps/pool/src/lib/channel_manager/mod.rs  # Should return empty
   ```

3. Verify channel creation updates:
   ```
   grep "data.current_pool_tag" pool-apps/pool/src/lib/channel_manager/mining_message_handler.rs  # Should find 2 occurrences
   ```

4. Verify API validation:
   ```
   grep -A 10 "pool_tag" pool-apps/pool/src/lib/api_server.rs | grep -E "(len|ascii)"
   ```
</verification>

<success_criteria>
- [ ] current_pool_tag field exists in ChannelManagerData
- [ ] pool_tag_string removed from ChannelManager struct
- [ ] current_pool_tag initialized from config.pool_signature() at startup
- [ ] StandardChannel creation reads from data.current_pool_tag
- [ ] ExtendedChannel creation reads from data.current_pool_tag
- [ ] /api/coinbase accepts optional pool_tag field
- [ ] pool_tag validates max 100 characters
- [ ] pool_tag validates ASCII graphic + space only
- [ ] pool_tag updates current_pool_tag when provided
- [ ] pool_tag uses existing value when omitted
- [ ] cargo check --package pool passes
</success_criteria>

<output>
After completion, create `.planning/quick/002-implement-pool-tag-feature-for-dynamic-c/002-SUMMARY.md`
</output>
