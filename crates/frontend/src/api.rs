//! Thin client for the compile-and-run backend.

use gloo_net::http::Request;
use playgraph_core::Playground;
use playgraph_core::api::{ExecuteResponse, GenerateResponse};

/// Base URL of the backend. Override at build time with `BACKEND_URL`? Kept
/// simple for now; the backend runs on this address by default.
pub const BACKEND: &str = "http://127.0.0.1:3001";

pub async fn execute(playground: &Playground) -> Result<ExecuteResponse, String> {
    let response = Request::post(&format!("{BACKEND}/api/execute"))
        .json(playground)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    response
        .json::<ExecuteResponse>()
        .await
        .map_err(|error| error.to_string())
}

#[allow(dead_code)]
pub async fn generate(playground: &Playground) -> Result<GenerateResponse, String> {
    let response = Request::post(&format!("{BACKEND}/api/generate"))
        .json(playground)
        .map_err(|error| error.to_string())?
        .send()
        .await
        .map_err(|error| error.to_string())?;
    response
        .json::<GenerateResponse>()
        .await
        .map_err(|error| error.to_string())
}
