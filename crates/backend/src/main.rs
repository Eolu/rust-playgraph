//! A small HTTP service that turns a [`Playground`] into a Rust program, runs
//! it with `cargo`, and returns the output.
//!
//! The service shells out to the system toolchain, so the machine running the
//! backend needs a Rust toolchain and the `directed` crate available (see
//! `DIRECTED_PATH`).

use std::path::PathBuf;
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
    println!("using directed crate at {}", directed_path().display());
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
    let directed = directed_path();
    if !directed.join("Cargo.toml").exists() {
        return Err(format!(
            "directed crate not found at {} (set DIRECTED_PATH)",
            directed.display()
        ));
    }

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
    let manifest = format!(
        "[package]\nname = \"playgraph-run\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\ndirected = {{ path = {:?} }}\n{num_traits}\n[workspace]\n",
        directed.display().to_string()
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

/// The `directed` crate used by generated code. Override with `DIRECTED_PATH`.
fn directed_path() -> PathBuf {
    if let Ok(path) = std::env::var("DIRECTED_PATH") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../directed/directed")
}

/// Shared cargo target directory, so `directed` is compiled only once.
fn cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PLAYGRAPH_CACHE") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.playgraph-cache")
}

fn temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("playgraph-{}-{nanos}", std::process::id()))
}
