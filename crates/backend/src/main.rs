//! A small HTTP service that turns a [`Playground`] into a Rust program, runs
//! it with `cargo`, and returns the output.
//!
//! The generated program depends on the published `directed` crate, so the
//! machine running the backend only needs a Rust toolchain and network access
//! to crates.io. During development the sibling `directed` checkout is used
//! automatically, and `DIRECTED_PATH` overrides that with an explicit path.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    Json, Router,
    routing::{get, post},
};
use playgraph_core::api::{ExecuteResponse, GenerateResponse};
use playgraph_core::{Playground, generate};
use tokio::process::Command;
use tower_http::cors::CorsLayer;

const EXECUTE_TIMEOUT: Duration = Duration::from_secs(180);

/// The published `directed` version used when no local checkout is available.
const DIRECTED_VERSION: &str = "0.4";

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/generate", post(generate_handler))
        .route("/api/execute", post(execute_handler))
        .layer(CorsLayer::permissive());

    let addr = std::env::var("PLAYGRAPH_ADDR").unwrap_or_else(|_| "127.0.0.1:3001".to_string());
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("failed to bind address");
    println!("playgraph-backend listening on http://{addr}");
    println!("directed dependency: {}", directed_dependency());
    axum::serve(listener, app).await.expect("server failed");
}

async fn generate_handler(Json(playground): Json<Playground>) -> Json<GenerateResponse> {
    Json(match generate(&playground) {
        Ok(generated) => GenerateResponse {
            ok: true,
            generated: Some(generated),
            error: None,
        },
        Err(error) => GenerateResponse {
            ok: false,
            generated: None,
            error: Some(error.to_string()),
        },
    })
}

async fn execute_handler(Json(playground): Json<Playground>) -> Json<ExecuteResponse> {
    Json(execute(playground).await)
}

async fn execute(playground: Playground) -> ExecuteResponse {
    let generated = match generate(&playground) {
        Ok(generated) => generated,
        Err(error) => return ExecuteResponse::failure(error.to_string(), None),
    };
    match compile_and_run(&generated).await {
        Ok(response) => response,
        Err(error) => ExecuteResponse::failure(error, Some(generated)),
    }
}

async fn compile_and_run(generated: &str) -> Result<ExecuteResponse, String> {
    let dir = temp_dir();
    tokio::fs::create_dir_all(dir.join("src"))
        .await
        .map_err(|error| error.to_string())?;

    // `num-traits` is only pulled in when the generated stages use it.
    let num_traits = if generated.contains("num_traits") {
        "num-traits = \"0.2\"\n"
    } else {
        ""
    };
    let directed = directed_dependency();
    let manifest = format!(
        "[package]\nname = \"playgraph-run\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\n{directed}\n{num_traits}\n[workspace]\n"
    );
    tokio::fs::write(dir.join("Cargo.toml"), manifest)
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::write(dir.join("src/main.rs"), generated)
        .await
        .map_err(|error| error.to_string())?;

    let target = cache_dir().join("target");
    tokio::fs::create_dir_all(&target)
        .await
        .map_err(|error| error.to_string())?;

    let started = Instant::now();
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(dir.join("Cargo.toml"))
        .current_dir(&dir)
        .env("CARGO_TARGET_DIR", &target)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = match tokio::time::timeout(EXECUTE_TIMEOUT, command.output()).await {
        Ok(Ok(output)) => output,
        Ok(Err(error)) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err(format!("failed to run cargo: {error}"));
        }
        Err(_) => {
            let _ = tokio::fs::remove_dir_all(&dir).await;
            return Err("compilation timed out".to_string());
        }
    };

    let duration_ms = started.elapsed().as_millis() as u64;
    let _ = tokio::fs::remove_dir_all(&dir).await;

    Ok(ExecuteResponse {
        ok: output.status.success(),
        generated: Some(generated.to_string()),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
        error: None,
        duration_ms,
    })
}

/// The `directed` dependency line for the generated crate's manifest.
///
/// `DIRECTED_PATH` wins if set. Otherwise a sibling `directed` checkout (as in
/// this repository) is used so local changes are picked up; when the backend is
/// installed from crates.io there is no sibling, so the published `directed`
/// version is used and no setup is required.
fn directed_dependency() -> String {
    let override_path = std::env::var("DIRECTED_PATH").ok();
    let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../directed/directed");
    directed_dependency_for(override_path.as_deref(), &sibling)
}

fn directed_dependency_for(override_path: Option<&str>, sibling: &Path) -> String {
    if let Some(path) = override_path {
        return format!("directed = {{ path = {path:?} }}");
    }
    if sibling.join("Cargo.toml").exists() {
        return format!(
            "directed = {{ path = {:?} }}",
            sibling.display().to_string()
        );
    }
    format!("directed = {DIRECTED_VERSION:?}")
}

/// Shared cargo target directory, so `directed` is compiled only once.
fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PLAYGRAPH_CACHE") {
        return PathBuf::from(dir);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // In a source checkout, keep the cache next to the workspace.
    if manifest_dir.join("../../Cargo.toml").exists() {
        return manifest_dir.join("../../.playgraph-cache");
    }
    // Installed from crates.io: the registry source tree is not a good place
    // for build artifacts, so use a per-user cache directory.
    user_cache_dir().join("playgraph")
}

fn user_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(dir);
    }
    #[cfg(windows)]
    if let Ok(dir) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(dir);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cache");
    }
    std::env::temp_dir()
}

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("playgraph-{}-{nanos}", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_path_wins() {
        let missing = Path::new("/nonexistent/directed");
        assert_eq!(
            directed_dependency_for(Some("/opt/directed"), missing),
            "directed = { path = \"/opt/directed\" }"
        );
    }

    #[test]
    fn falls_back_to_registry_without_a_checkout() {
        let missing = Path::new("/nonexistent/directed");
        assert_eq!(
            directed_dependency_for(None, missing),
            format!("directed = {DIRECTED_VERSION:?}")
        );
    }

    #[test]
    fn uses_the_sibling_checkout_when_present() {
        let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../directed/directed");
        if sibling.join("Cargo.toml").exists() {
            assert!(directed_dependency_for(None, &sibling).contains("path"));
        }
    }
}
