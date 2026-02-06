//! Request and response types for the HTTP API.
//!
//! This module defines the data structures used for coinbase update requests
//! and responses, including validation logic for API-03 constraints.

use serde::{Deserialize, Serialize};

/// Request to update the pool's coinbase output.
///
/// ## Validation
///
/// - `user_id` must be <= 128 characters (API-03)
/// - `user_id` must contain only alphanumeric characters, underscores, and hyphens (API-03)
/// - `pool_tag` must be <= 100 characters (scriptSig space constraint)
/// - `pool_tag` must contain only ASCII graphic characters and spaces
/// - `address` validation occurs in handler layer using bitcoin crate
#[derive(Debug, Clone, Deserialize)]
pub struct CoinbaseUpdateRequest {
    /// Bitcoin address to receive coinbase rewards
    pub address: String,
    /// User identifier for share attribution
    pub user_id: String,
    /// Optional pool identification string for coinbase scriptSig
    #[serde(default)]
    pub pool_tag: Option<String>,
}

impl CoinbaseUpdateRequest {
    /// Validates request constraints per API-03 specification.
    ///
    /// ## Returns
    ///
    /// - `Ok(())` if validation passes
    /// - `Err(String)` with descriptive error message if validation fails
    ///
    /// ## Validation Rules
    ///
    /// - user_id length must not exceed 128 characters
    /// - user_id must contain only: alphanumeric, underscore, hyphen
    /// - pool_tag length must not exceed 100 characters (scriptSig space constraint)
    /// - pool_tag must contain only ASCII graphic characters and spaces
    pub fn validate(&self) -> Result<(), String> {
        // Validate user_id length (API-03)
        if self.user_id.len() > 128 {
            return Err(format!(
                "user_id exceeds maximum length of 128 characters (got {})",
                self.user_id.len()
            ));
        }

        // Validate user_id characters (API-03)
        if !self
            .user_id
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(
                "user_id contains invalid characters (only alphanumeric, underscore, and hyphen allowed)"
                    .to_string(),
            );
        }

        // Validate pool_tag if provided
        if let Some(tag) = &self.pool_tag {
            // Max 100 characters per PRD line 131
            if tag.len() > 100 {
                return Err(format!(
                    "pool_tag exceeds maximum length of 100 characters (got {})",
                    tag.len()
                ));
            }
            // ASCII graphic + space per PRD line 588
            if !tag.chars().all(|c| c.is_ascii_graphic() || c == ' ') {
                return Err(
                    "pool_tag contains invalid characters (only ASCII graphic and space allowed)"
                        .to_string(),
                );
            }
        }

        Ok(())
    }
}

/// Response from coinbase update operation.
#[derive(Debug, Clone, Serialize)]
pub struct CoinbaseUpdateResponse {
    /// Whether the operation succeeded
    pub success: bool,
    /// Human-readable message describing the result
    pub message: String,
}

impl CoinbaseUpdateResponse {
    /// Creates a success response with the given message.
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

    /// Creates an error response with the given message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

/// Request to update the pool's shares_per_minute target.
///
/// ## Validation
///
/// - `shares_per_minute` must be positive
/// - `shares_per_minute` must be between 0.1 and 60.0 (reasonable range)
#[derive(Debug, Serialize, Deserialize)]
pub struct SharesPerMinuteUpdateRequest {
    pub shares_per_minute: f32,
}

impl SharesPerMinuteUpdateRequest {
    /// Validates request constraints.
    ///
    /// ## Returns
    ///
    /// - `Ok(())` if validation passes
    /// - `Err(String)` with descriptive error message if validation fails
    ///
    /// ## Validation Rules
    ///
    /// - shares_per_minute must be positive
    /// - shares_per_minute must be >= 0.1 (minimum reasonable difficulty)
    /// - shares_per_minute must be <= 60.0 (maximum reasonable difficulty)
    pub fn validate(&self) -> Result<(), String> {
        // shares_per_minute must be positive and reasonable (0.1 to 60.0)
        if self.shares_per_minute <= 0.0 {
            return Err("shares_per_minute must be positive".to_string());
        }
        if self.shares_per_minute > 60.0 {
            return Err("shares_per_minute exceeds maximum (60.0)".to_string());
        }
        if self.shares_per_minute < 0.1 {
            return Err("shares_per_minute below minimum (0.1)".to_string());
        }
        Ok(())
    }
}

/// Response from shares_per_minute update operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct SharesPerMinuteUpdateResponse {
    /// Whether the operation succeeded
    pub success: bool,
    /// Human-readable message describing the result
    pub message: String,
}

impl SharesPerMinuteUpdateResponse {
    /// Creates a success response with the given message.
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

    /// Creates an error response with the given message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_user_id_alphanumeric() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "user123".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_valid_user_id_with_underscore() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "valid_user_name".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_valid_user_id_with_hyphen() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "valid-user-name".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_valid_user_id_mixed_valid_chars() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "User_123-test".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_valid_user_id_empty() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "".to_string(),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_valid_user_id_exactly_128_chars() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "a".repeat(128),
        };
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_user_id_too_long() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "a".repeat(129),
        };
        let result = req.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("exceeds maximum length of 128 characters"));
    }

    #[test]
    fn test_user_id_invalid_chars_at_symbol() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "invalid@user".to_string(),
        };
        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid characters"));
    }

    #[test]
    fn test_user_id_invalid_chars_dollar() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "user$name".to_string(),
        };
        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid characters"));
    }

    #[test]
    fn test_user_id_invalid_chars_space() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "user name".to_string(),
        };
        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid characters"));
    }

    #[test]
    fn test_user_id_invalid_chars_period() {
        let req = CoinbaseUpdateRequest {
            address: "bcrt1qtest".to_string(),
            user_id: "user.name".to_string(),
        };
        let result = req.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid characters"));
    }

    #[test]
    fn test_response_success_constructor() {
        let resp = CoinbaseUpdateResponse::success("Operation completed");
        assert!(resp.success);
        assert_eq!(resp.message, "Operation completed");
    }

    #[test]
    fn test_response_error_constructor() {
        let resp = CoinbaseUpdateResponse::error("Operation failed");
        assert!(!resp.success);
        assert_eq!(resp.message, "Operation failed");
    }

    #[test]
    fn test_response_success_with_string() {
        let resp = CoinbaseUpdateResponse::success("Test".to_string());
        assert!(resp.success);
        assert_eq!(resp.message, "Test");
    }

    #[test]
    fn test_response_error_with_string() {
        let resp = CoinbaseUpdateResponse::error("Error".to_string());
        assert!(!resp.success);
        assert_eq!(resp.message, "Error");
    }
}
