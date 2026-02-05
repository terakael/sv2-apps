//! HTTP API module for dynamic coinbase switching.
//!
//! This module provides HTTP endpoints for updating the pool's coinbase outputs
//! at runtime, enabling sub-100ms address rotation for time-shared mining.
//!
//! ## Structure
//!
//! - `types`: Request/response types with validation
//! - `handlers`: HTTP endpoint handlers (added in Plan 04)
//!
//! ## Endpoints
//!
//! - `POST /api/coinbase`: Update coinbase address and user_id

pub mod types;

pub use types::{CoinbaseUpdateRequest, CoinbaseUpdateResponse};
