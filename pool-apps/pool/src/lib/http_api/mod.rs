//! HTTP API module for dynamic coinbase switching.
//!
//! This module provides HTTP endpoints for updating the pool's coinbase outputs
//! at runtime, enabling sub-100ms address rotation for time-shared mining.
//!
//! ## Structure
//!
//! - `types`: Request/response types with validation
//! - `handlers`: HTTP endpoint handlers
//!
//! ## Endpoints
//!
//! - `POST /api/coinbase`: Update coinbase address and user_id
//!
//! ## Usage
//!
//! ```rust,ignore
//! use pool::http_api;
//! use pool::channel_manager::ChannelManager;
//!
//! let router = http_api::create_router(channel_manager);
//! let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
//! axum::serve(listener, router).await?;
//! ```

use std::net::SocketAddr;

use axum::{Router, routing::post};

use crate::channel_manager::ChannelManager;

pub mod handlers;
pub mod types;

pub use types::{CoinbaseUpdateRequest, CoinbaseUpdateResponse, SharesPerMinuteUpdateRequest, SharesPerMinuteUpdateResponse};

/// Creates the HTTP API router with all endpoints configured.
///
/// # Arguments
/// * `channel_manager` - ChannelManager instance to share across handlers
///
/// # Returns
/// Configured axum Router ready to serve
///
/// # Endpoints
/// - `POST /api/coinbase` - Update coinbase address (see handlers::handle_coinbase_update)
/// - `POST /update-shares-per-minute` - Update shares_per_minute target (see handlers::handle_shares_per_minute_update)
pub fn create_router(channel_manager: ChannelManager) -> Router {
    Router::new()
        .route("/api/coinbase", post(handlers::handle_coinbase_update))
        .route("/update-shares-per-minute", post(handlers::handle_shares_per_minute_update))
        .with_state(channel_manager)
}

/// Starts the HTTP API server on the given bind address.
///
/// # Arguments
/// * `channel_manager` - ChannelManager instance for API handlers
/// * `bind_addr` - Socket address to bind the server (e.g., "127.0.0.1:8080")
///
/// # Returns
/// - `Ok(())` when server shuts down gracefully
/// - `Err` if server fails to bind or encounters fatal error
///
/// # Example
/// ```rust,ignore
/// let addr: SocketAddr = "127.0.0.1:8080".parse()?;
/// start_api_server(channel_manager, addr).await?;
/// ```
pub async fn start_api_server(
    channel_manager: ChannelManager,
    bind_addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let router = create_router(channel_manager);
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;

    tracing::info!("API server listening on {}", bind_addr);
    axum::serve(listener, router).await?;
    Ok(())
}
