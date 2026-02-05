//! ## Webhook Notification Module
//!
//! Provides HTTP webhook notifications for share validation events.
//!
//! This module implements the webhook infrastructure for notifying external systems
//! when miners submit valid shares or find blocks. It uses a fire-and-forget pattern
//! (tokio::spawn) to ensure webhook delivery never blocks pool operations.
//!
//! ## Architecture
//!
//! - [`WebhookClient`]: HTTP client with connection pooling for webhook delivery
//! - [`ShareWebhookPayload`]: Typed payload containing share details and user attribution
//!
//! ## Non-blocking Delivery
//!
//! Webhooks are delivered via tokio::spawn (fire-and-forget). This ensures:
//! - Share validation never waits for HTTP responses (WEBHOOK-08)
//! - Webhook failures are logged but don't crash pool (WEBHOOK-09)
//! - Connection pooling reuses HTTP connections for efficiency
//!
//! ## Usage
//!
//! ```rust,ignore
//! let client = WebhookClient::new("http://localhost:3000/webhook".to_string());
//!
//! // In share validation handler:
//! let payload = ShareWebhookPayload {
//!     user_id: "alice".to_string(),
//!     share_hash: "abc123...".to_string(),
//!     difficulty: 1024.0,
//!     job_id: 42,
//!     timestamp: chrono::Utc::now().timestamp(),
//!     is_block: false,
//!     channel_id: 1,
//!     downstream_id: 0,
//!     sequence_number: 100,
//! };
//! client.send_share_notification(payload); // Returns immediately
//! ```

pub mod client;
pub mod types;

pub use client::WebhookClient;
pub use types::ShareWebhookPayload;
