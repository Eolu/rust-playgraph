# rust-playgraph

A visual playground for the [`directed`](https://github.com/Eolu/directed) graph engine. Write each
node as a plain Rust function, compose them on a canvas, and run the graph with
the real Rust compiler.

![example](./example.png)  

This largely exists as a demo project to show what `directed` can do. But if I discover a more
prudent use of it I might be willing to expand it down one path or another.  

## Running

Prerequisites: a Rust toolchain with the `wasm32-unknown-unknown` target and
[`trunk`](https://trunkrs.dev) for the frontend.

```sh
# 1. backend (compiles/runs submitted graphs)
cargo run -p playgraph-backend        # http://127.0.0.1:3001

# 2. frontend (run from the repo root or from crates/frontend)
trunk serve --open                    # http://127.0.0.1:8080
```

The root `Trunk.toml` points Trunk at `crates/frontend`, so `trunk serve` works
from either location. The backend does **not** serve the UI — it only exposes
`/api/*`; load the app from the Trunk dev server.

The generated program depends on the published
[`directed`](https://crates.io/crates/directed) crate, so an installed backend
only needs a Rust toolchain (plus crates.io access on the first run). When run
from this repository the sibling `directed` checkout is used automatically, so
local changes are picked up; set `DIRECTED_PATH=/path/to/directed/directed` to
point at a different checkout. Compiled artifacts are cached in
`.playgraph-cache/` during development, or in a per-user cache directory when the
backend is installed.

## Security

The backend runs arbitrary Rust code submitted by the browser. It is intended
for local development only; do not expose it to untrusted users without
sandboxing (the real rust-playground uses seccomp/container isolation).

## Roadmap

- Node-level metadata overrides (per node, not just per stage).
- Inline compile diagnostics mapped back to the editor.
- Share a playground by URL.
