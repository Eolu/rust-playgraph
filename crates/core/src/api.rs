//! HTTP request/response types shared by the frontend and backend.

use serde::{Deserialize, Serialize};

/// Result of a code-generation-only request (no compilation).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GenerateResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of compiling and running a playground.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExecuteResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated: Option<String>,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
}

impl ExecuteResponse {
    pub fn failure(error: impl Into<String>, generated: Option<String>) -> Self {
        Self {
            ok: false,
            generated,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            error: Some(error.into()),
            duration_ms: 0,
        }
    }
}
