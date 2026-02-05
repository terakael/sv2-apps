//! ## Webhook Payload Types
//!
//! Defines the webhook payload structures for share notifications.

use serde::Serialize;

/// Webhook payload for share validation events.
///
/// This payload is sent to external systems when miners submit valid shares
/// or find blocks. It includes attribution (user_id), share details, and
/// channel information for complete traceability.
///
/// ## Required Fields (WEBHOOK-02 through WEBHOOK-05)
///
/// - `user_id`: User attribution from job-to-user mapping (WEBHOOK-02)
/// - `share_hash`: Hex-encoded share hash (WEBHOOK-03)
/// - `difficulty`: Share difficulty as float (WEBHOOK-03)
/// - `job_id`: Job ID that the share was submitted for (WEBHOOK-03)
/// - `timestamp`: Unix timestamp when share was validated (WEBHOOK-03)
/// - `is_block`: True for BlockFound, false for Valid shares (WEBHOOK-04)
/// - `channel_id`: Channel ID that submitted the share (WEBHOOK-05)
/// - `downstream_id`: Downstream ID (mining connection) (WEBHOOK-05)
/// - `sequence_number`: Sequence number of the share submission (WEBHOOK-05)
#[derive(Serialize, Clone, Debug)]
pub struct ShareWebhookPayload {
    /// User identifier from coinbase update API call.
    ///
    /// Extracted from job_id → user_id mapping maintained in ChannelManagerData.
    /// May be "unknown" if job mapping expired or missing.
    pub user_id: String,

    /// Share hash in hex format.
    ///
    /// The hash that satisfies the target difficulty, proving work was done.
    pub share_hash: String,

    /// Difficulty of the share.
    ///
    /// Target difficulty for this job, typically higher than network difficulty
    /// to reduce pool communication overhead.
    pub difficulty: f64,

    /// Job ID that the share was submitted for.
    ///
    /// Links the share back to a specific mining job. Used for lagging share
    /// attribution after coinbase switches.
    pub job_id: u32,

    /// Unix timestamp when share was validated.
    ///
    /// Records when the pool validated the share, not when the miner found it.
    /// Used for time-based analytics and accounting.
    pub timestamp: i64,

    /// True for BlockFound, false for Valid shares.
    ///
    /// Distinguishes block solutions (submitted to network) from valid shares
    /// (used for difficulty adjustment and payouts).
    pub is_block: bool,

    /// Channel ID that submitted the share.
    ///
    /// Identifies the specific channel (standard or extended) within a downstream
    /// connection. Used for per-channel analytics.
    pub channel_id: u32,

    /// Downstream ID (mining connection).
    ///
    /// Identifies the downstream connection (miner) within the pool's connection map.
    /// Unique per connected miner.
    pub downstream_id: usize,

    /// Sequence number of the share submission.
    ///
    /// Monotonically increasing counter for share submissions within a channel.
    /// Used for detecting missing or duplicate shares.
    pub sequence_number: u32,
}
