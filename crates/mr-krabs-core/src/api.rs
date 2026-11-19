//! Serializable DTOs and errors that cross the shell boundary.
//! Field names are camelCase on the wire to match TypeScript conventions.

use std::fmt::Display;

use mrk_money::Money;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Validation,
    NotFound,
    UnknownCommand,
    Internal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn validation(e: impl Display) -> Self {
        Self::new(ErrorCode::Validation, e.to_string())
    }
    pub fn internal(e: impl Display) -> Self {
        Self::new(ErrorCode::Internal, e.to_string())
    }
}

impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    pub status: String,
    pub version: String,
    pub schema_version: u32,
    pub latest_schema_version: u32,
    pub db_path: Option<String>,
}

/// Money on the wire: minor units as a **string** (no JS number precision issues)
/// plus a display string formatted in Rust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoneyDto {
    pub minor: String,
    pub currency: String,
    pub display: String,
}

impl MoneyDto {
    pub fn from_money(m: Money) -> Self {
        Self {
            minor: m.minor.to_string(),
            currency: m.currency.to_string(),
            display: m.display_es_cl(),
        }
    }
}

/// User-typed money as text; parsed in Rust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoneyInput {
    pub amount: String,
    pub currency: String,
}
