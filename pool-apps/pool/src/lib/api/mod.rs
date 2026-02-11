use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::Arc};
use stratum_apps::stratum_core::bitcoin::{script::ScriptBuf, Address};

use crate::channel_manager::ChannelManager;

fn default_max_shares() -> Option<u64> {
    Some(50)
}

#[derive(Deserialize)]
pub struct AssignUserRequest {
    pub user_id: String,
    pub coinbase_address: String,
    #[serde(default)]
    pub coinbase_prefix_tag: Option<String>,
    #[serde(default = "default_max_shares")]
    pub max_shares: Option<u64>,
}

#[derive(Serialize)]
pub struct AssignUserResponse {
    pub success: bool,
    pub assigned_channel: Option<String>,
    pub message: String,
}

/// Creates management routes for the pool HTTP API.
///
/// Currently provides:
/// - POST /api/assign-user: Assign a user to a channel with custom coinbase address
pub fn management_routes(channel_manager: Arc<ChannelManager>) -> Router {
    Router::new()
        .route("/api/assign-user", post(assign_user_handler))
        .with_state(channel_manager)
}

async fn assign_user_handler(
    State(channel_manager): State<Arc<ChannelManager>>,
    Json(req): Json<AssignUserRequest>,
) -> (StatusCode, Json<AssignUserResponse>) {
    let address = match Address::from_str(&req.coinbase_address) {
        Ok(addr) => addr.assume_checked(),
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AssignUserResponse {
                    success: false,
                    assigned_channel: None,
                    message: format!("Invalid address: {}", e),
                }),
            );
        }
    };

    let script_pubkey: ScriptBuf = address.script_pubkey();
    let tag = req
        .coinbase_prefix_tag
        .unwrap_or_else(|| req.user_id.clone());

    match channel_manager
        .assign_user(req.user_id.clone(), script_pubkey, tag, req.max_shares)
        .await
    {
        Ok(handle) => (
            StatusCode::OK,
            Json(AssignUserResponse {
                success: true,
                assigned_channel: Some(handle.to_string()),
                message: format!("User assigned to channel {}", handle),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(AssignUserResponse {
                success: false,
                assigned_channel: None,
                message: format!("Assignment failed: {:?}", e),
            }),
        ),
    }
}
