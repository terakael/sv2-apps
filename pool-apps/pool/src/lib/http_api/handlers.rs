//! HTTP endpoint handlers for coinbase update API.
//!
//! This module contains the axum handlers that process HTTP requests
//! and call ChannelManager methods to update pool state.

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tracing::{error, info};

use crate::channel_manager::ChannelManager;
use super::types::{CoinbaseUpdateRequest, CoinbaseUpdateResponse, SharesPerMinuteUpdateRequest, SharesPerMinuteUpdateResponse};

/// Handles POST /api/coinbase requests to update the pool's coinbase output address.
///
/// # Request Flow
/// 1. Validates user_id constraints (API-03)
/// 2. Calls ChannelManager to validate Bitcoin address and update state (API-02)
/// 3. ChannelManager recreates jobs and broadcasts to miners (TMPL-03, TMPL-04)
///
/// # Response Codes
/// - 200: Success - coinbase updated and jobs broadcast
/// - 400: Bad request - invalid address or user_id validation failed
/// - 500: Internal error - pool error during update
///
/// # Example
/// ```bash
/// curl -X POST http://localhost:8080/api/coinbase \
///   -H "Content-Type: application/json" \
///   -d '{"address": "bcrt1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh", "user_id": "user_123"}'
/// ```
pub async fn handle_coinbase_update(
    State(channel_manager): State<ChannelManager>,
    Json(req): Json<CoinbaseUpdateRequest>,
) -> impl IntoResponse {
    // Phase 1: Validate request (API-03)
    if let Err(e) = req.validate() {
        info!("Coinbase update rejected: {}", e);
        return (
            StatusCode::BAD_REQUEST,
            Json(CoinbaseUpdateResponse::error(e)),
        );
    }

    // Log request (first 8 chars of address for privacy)
    let address_preview = &req.address[..8.min(req.address.len())];
    info!("Processing coinbase update: address={}..., user_id={}", address_preview, req.user_id);

    // Phase 2: Call ChannelManager update (validates address - API-02)
    match channel_manager.update_coinbase_and_broadcast(&req.address, req.user_id.clone(), req.pool_tag.clone()).await {
        Ok(_) => {
            info!("Coinbase update successful for user_id={}", req.user_id);
            (
                StatusCode::OK,
                Json(CoinbaseUpdateResponse::success("Coinbase updated successfully")),
            )
        }
        Err(e) => {
            // Check error message to determine status code
            let error_msg = e.to_string();
            let status = if error_msg.contains("Invalid Bitcoin address") || error_msg.contains("Address") {
                StatusCode::BAD_REQUEST  // Address validation failed
            } else {
                StatusCode::INTERNAL_SERVER_ERROR  // Pool error
            };

            error!("Coinbase update failed for user_id={}: {}", req.user_id, error_msg);
            (
                status,
                Json(CoinbaseUpdateResponse::error(
                    if status == StatusCode::BAD_REQUEST {
                        error_msg
                    } else {
                        "Internal server error".to_string()
                    }
                )),
            )
        }
    }
}

/// Handles POST /update-shares-per-minute requests to update mining difficulty target.
///
/// # Request Flow
/// 1. Validates shares_per_minute constraints (positive, reasonable range)
/// 2. Calls ChannelManager to update all channels immediately
/// 3. ChannelManager recalculates targets and broadcasts SetTarget messages
///
/// # Response Codes
/// - 200: Success - shares_per_minute updated and targets recalculated
/// - 400: Bad request - invalid shares_per_minute value
/// - 500: Internal error - pool error during update
///
/// # Example
/// ```bash
/// # 3 active users, distribute 6.0 shares/min: 6.0 / 3 = 2.0
/// curl -X POST http://localhost:8080/update-shares-per-minute \
///   -H "Content-Type: application/json" \
///   -d '{"shares_per_minute": 2.0}'
/// ```
pub async fn handle_shares_per_minute_update(
    State(channel_manager): State<ChannelManager>,
    Json(req): Json<SharesPerMinuteUpdateRequest>,
) -> impl IntoResponse {
    // Phase 1: Validate request
    if let Err(e) = req.validate() {
        info!("Shares per minute update rejected: {}", e);
        return (
            StatusCode::BAD_REQUEST,
            Json(SharesPerMinuteUpdateResponse::error(e)),
        );
    }

    info!("Processing shares_per_minute update: {}", req.shares_per_minute);

    // Phase 2: Call ChannelManager update
    match channel_manager.update_shares_per_minute(req.shares_per_minute).await {
        Ok(_) => {
            info!("Shares per minute update successful: {}", req.shares_per_minute);
            (
                StatusCode::OK,
                Json(SharesPerMinuteUpdateResponse::success(
                    format!("Shares per minute updated to {}", req.shares_per_minute)
                )),
            )
        }
        Err(e) => {
            let error_msg = e.to_string();
            error!("Shares per minute update failed: {}", error_msg);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SharesPerMinuteUpdateResponse::error("Internal server error")),
            )
        }
    }
}
