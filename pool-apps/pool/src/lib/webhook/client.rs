//! ## Webhook HTTP Client
//!
//! Implements the HTTP client for webhook notifications with fire-and-forget delivery.

use reqwest::Client;
use std::time::Duration;
use tokio::spawn;
use tracing::{error, info, instrument};

use super::types::ShareWebhookPayload;

/// HTTP client for webhook notifications.
///
/// Uses reqwest with connection pooling to efficiently deliver webhook payloads
/// to external systems. All deliveries are fire-and-forget (tokio::spawn) to
/// ensure webhook failures never block pool operations.
///
/// ## Non-blocking Delivery (WEBHOOK-08)
///
/// The `send_share_notification` method spawns a background task and returns
/// immediately. This ensures share validation never waits for HTTP responses,
/// maintaining pool responsiveness under webhook failures or network issues.
///
/// ## Error Handling (WEBHOOK-09)
///
/// Webhook failures are logged via tracing at error level but don't propagate
/// to the caller. This prevents webhook issues from crashing the pool while
/// maintaining observability through logs.
#[derive(Clone)]
pub struct WebhookClient {
    client: Client,
    webhook_url: String,
}

impl WebhookClient {
    /// Creates a new webhook client.
    ///
    /// Configures the underlying reqwest client with:
    /// - 5-second timeout to prevent hanging
    /// - Connection pooling (10 idle connections per host)
    ///
    /// # Arguments
    ///
    /// * `webhook_url` - Full URL for webhook delivery (e.g., "http://localhost:3000/webhook")
    pub fn new(webhook_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5)) // Prevent hanging on slow endpoints
            .pool_max_idle_per_host(10)      // Reuse connections for efficiency
            .build()
            .expect("Failed to create webhook client");

        Self {
            client,
            webhook_url,
        }
    }

    /// Sends webhook notification in background (fire-and-forget).
    ///
    /// Spawns a tokio task to deliver the webhook, returning immediately without
    /// waiting for HTTP response. This ensures webhook delivery never blocks the
    /// share validation path.
    ///
    /// ## Non-blocking Pattern (WEBHOOK-08)
    ///
    /// ```rust,ignore
    /// // Returns immediately, doesn't wait for HTTP response
    /// webhook_client.send_share_notification(payload);
    /// // Continue share validation...
    /// ```
    ///
    /// ## Error Logging (WEBHOOK-09)
    ///
    /// Failures are logged at error level with context (status code, error details)
    /// but don't crash the pool or block subsequent operations.
    ///
    /// # Arguments
    ///
    /// * `payload` - Share webhook payload to deliver
    #[instrument(skip(self, payload), fields(
        user_id = %payload.user_id,
        job_id = payload.job_id,
        is_block = payload.is_block
    ))]
    pub fn send_share_notification(&self, payload: ShareWebhookPayload) {
        let client = self.client.clone();
        let url = self.webhook_url.clone();

        // WEBHOOK-08: Fire-and-forget pattern (non-blocking)
        spawn(async move {
            match client.post(&url)
                .json(&payload)
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Webhook delivered successfully");
                    } else {
                        // WEBHOOK-09: Log failure but don't block pool
                        error!(
                            status = %response.status(),
                            "Webhook delivery failed with non-2xx status"
                        );
                    }
                }
                Err(e) => {
                    // WEBHOOK-09: Log failure but don't block pool
                    error!(error = %e, "Webhook delivery failed");
                }
            }
        });

        // Return immediately, don't wait for HTTP response
    }
}
